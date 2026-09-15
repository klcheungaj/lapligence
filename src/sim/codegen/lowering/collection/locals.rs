//! Locals.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn signal_dependency(&self, info: &SignalInfo) -> IrDependency {
        self.reference_dependency(info)
    }

    /// Register storage for a procedural declaration.
    ///
    /// Automatic variables remain lexical C locals. Static variables become
    /// hidden model signals so they retain values across block reentry and
    /// participate in typed optimizer read/write accounting.
    pub(in super::super) fn collect_loop_var(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<ProcLocalInfo, String> {
        let lifetime = self.db.variable_lifetime(node);
        match lifetime {
            VariableLifetime::Automatic => {
                if let Some(info) = self.proc_locals.get(&node) {
                    return Ok(info.clone());
                }
            }
            VariableLifetime::Static => {
                if let Some(info) = self.proc_local_instances.get(&(self.inst, node)) {
                    let info = info.clone();
                    self.proc_locals.insert(node, info.clone());
                    return Ok(info);
                }
            }
            VariableLifetime::Unavailable => {}
        }
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => ty.clone(),
            other => {
                return Err(format!(
                    "unsupported procedural loop declaration in `{path}` (node kind {other:?})"
                ))
            }
        };
        let real = is_real_kind(&ty.kind);
        let width = if real {
            0
        } else {
            self.signal_width(path, &self.node(node).name, &ty)?
        };
        let (c_name, static_signal) = match lifetime {
            VariableLifetime::Automatic => (format!("_lv{}", node.index()), None),
            VariableLifetime::Static => {
                let ir = self.model.signals.len();
                let signal = SignalInfo {
                    global: format!("_ls{}_{}", self.inst.index(), node.index()),
                    width,
                    signed: ty.signed,
                    two_state: self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind),
                    real,
                    shortreal: real && ty.kind == "shortreal",
                    net_driver: None,
                    ir,
                };
                self.model.signals.push(IrSignal {
                    c_name: signal.global.clone(),
                    hdl_name: None,
                    ty: if real {
                        IrType::Real {
                            shortreal: ty.kind == "shortreal",
                        }
                    } else {
                        IrType::Packed {
                            width: signal.width,
                            signed: signal.signed,
                            two_state: signal.two_state,
                        }
                    },
                    net_driver: None,
                    net_alias: Vec::new(),
                    alias: None,
                    omit: false,
                });
                if let Some(initializer) = self.db.var_initializer(node) {
                    let lowered = self.lower_declaration_initializer(
                        path,
                        node,
                        initializer,
                        IrInitTarget::Signal(signal.ir),
                        signal.width,
                        signal.signed,
                        signal.two_state,
                        signal.real,
                    );
                    match lowered {
                        Ok(initializer) => self.declaration_inits.push(initializer),
                        Err(lowering_error) => {
                            let value = self
                                .var_decl_init(path, &self.node(node).name, initializer)
                                .map_err(|_| lowering_error)?;
                            self.var_inits.push((signal.clone(), value));
                        }
                    }
                }
                (signal.global.clone(), Some(signal))
            }
            VariableLifetime::Unavailable => {
                return Err(format!(
                    "resolved lifetime is unavailable for procedural variable `{}` in `{path}`",
                    self.node(node).name
                ));
            }
        };
        let info = ProcLocalInfo {
            c_name,
            width,
            signed: ty.signed,
            two_state: if real {
                false
            } else {
                self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind)
            },
            static_signal,
        };
        if matches!(lifetime, VariableLifetime::Static) {
            self.proc_local_instances
                .insert((self.inst, node), info.clone());
        }
        self.proc_locals.insert(node, info.clone());
        Ok(info)
    }

    /// Register an automatic native-string foreach iterator. Strings use the
    /// object ABI rather than packed/real `ProcLocalInfo`; the generated name
    /// is still declaration-derived so nested loop scopes cannot collide.
    pub(in super::super) fn collect_loop_string_var(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<String, String> {
        if let Some(name) = self.proc_string_locals.get(&node) {
            return Ok(name.clone());
        }
        let is_string = matches!(
            self.kind(node),
            NodeKind::Var { ty } if ty.kind == "string"
        );
        if !is_string {
            return Err(format!(
                "foreach string iterator `{}` in `{path}` is not a string variable",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Automatic {
            return Err(format!(
                "string foreach iterator `{}` in `{path}` must have automatic lifetime",
                self.node(node).name
            ));
        }
        let name = format!("_lv{}", node.index());
        self.proc_string_locals.insert(node, name.clone());
        Ok(name)
    }

    pub(in super::super) fn proc_string_local_name(&self, node: NodeId) -> Option<&str> {
        self.proc_string_locals.get(&node).map(String::as_str)
    }

    /// Register an automatic mailbox declared in a process body. Mailboxes
    /// are native runtime pointers and therefore cannot use packed local
    /// storage. A declaration-derived name keeps nested activations distinct
    /// without relying on source spelling.
    pub(in super::super) fn collect_mailbox_local(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<String, String> {
        if let Some(name) = self.proc_mailbox_locals.get(&node) {
            return Ok(name.clone());
        }
        if !self.is_mailbox_expr(path, node) {
            return Err(format!(
                "procedural mailbox declaration `{}` in `{path}` has no mailbox type",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Automatic {
            return Err(format!(
                "process-local mailbox `{}` in `{path}` is not automatic",
                self.node(node).name
            ));
        }
        let name = format!("_lm{}", node.index());
        self.proc_mailbox_locals.insert(node, name.clone());
        Ok(name)
    }

    /// Register the model-backed identity of a static mailbox declared in a
    /// process body. Its declaration-time constructor is emitted with the
    /// other time-zero mailbox initializers.
    pub(in super::super) fn collect_mailbox_static_object(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<usize, String> {
        if let Some(index) = self.proc_mailbox_static_objects.get(&(self.inst, node)) {
            return Ok(*index);
        }
        if !self.is_mailbox_expr(path, node) {
            return Err(format!(
                "procedural mailbox declaration `{}` in `{path}` has no mailbox type",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Static {
            return Err(format!(
                "process-local mailbox `{}` in `{path}` is not static",
                self.node(node).name
            ));
        }
        let index = self.model.objects.len();
        self.model.objects.push(crate::sim::ir::IrObject {
            c_name: format!("O_M{}_L{}", self.inst.index(), node.index()),
            ty: IrObjectType::Chandle,
            initial: None,
        });
        if let Some(initializer) = self.db.var_initializer(node) {
            self.mailbox_object_initializers
                .push((node, index, initializer, path.to_owned()));
        }
        self.proc_mailbox_static_objects
            .insert((self.inst, node), index);
        Ok(index)
    }

    pub(in super::super) fn proc_mailbox_local_name(&self, node: NodeId) -> Option<&str> {
        self.proc_mailbox_locals.get(&node).map(String::as_str)
    }

    /// Resolve a process-local mailbox through lexical begin/loop scopes.
    /// Owned references normally carry a target declaration; the name walk is
    /// retained for frontend references whose target was not captured.
    pub(in super::super) fn lexical_proc_mailbox_local(&self, reference: NodeId) -> Option<(NodeId, &str)> {
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(reference)
        {
            if let Some(name) = self.proc_mailbox_local_name(*target) {
                return Some((*target, name));
            }
        }
        if let Some(name) = self.proc_mailbox_local_name(reference) {
            return Some((reference, name));
        }
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    self.is_mailbox_expr("", **child) && self.node(**child).name == name
                }) {
                    if let Some(c_name) = self.proc_mailbox_local_name(*variable) {
                        return Some((*variable, c_name));
                    }
                }
            }
            let variable = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .find(|variable| {
                        self.is_mailbox_expr("", **variable) && self.node(**variable).name == name
                    })
                    .copied(),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .find(|variable| {
                        self.is_mailbox_expr("", **variable) && self.node(**variable).name == name
                    })
                    .copied(),
                _ => None,
            };
            if let Some(variable) = variable {
                if let Some(c_name) = self.proc_mailbox_local_name(variable) {
                    return Some((variable, c_name));
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    /// Register a process handle declared in a procedural body. Automatic
    /// handles live in the activation's C frame; static handles use a model
    /// object so their identity survives repeated activations.
    pub(in super::super) fn collect_process_local(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<ProcessTarget, String> {
        if let Some(name) = self.proc_process_locals.get(&node) {
            return Ok(ProcessTarget::Local(name.clone()));
        }
        if let Some(index) = self.proc_process_static_objects.get(&(self.inst, node)) {
            return Ok(ProcessTarget::Object(*index));
        }
        let is_process = matches!(
            self.kind(node),
            NodeKind::Var { ty } if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
        );
        if !is_process {
            return Err(format!(
                "procedural process declaration `{}` in `{path}` has a non-process type",
                self.node(node).name
            ));
        }
        match self.db.variable_lifetime(node) {
            VariableLifetime::Automatic => {
                let name = format!("_lp{}", node.index());
                self.proc_process_locals.insert(node, name.clone());
                Ok(ProcessTarget::Local(name))
            }
            VariableLifetime::Static => {
                let index = self.collect_process_static_object(path, node)?;
                Ok(ProcessTarget::Object(index))
            }
            VariableLifetime::Unavailable => Err(format!(
                "resolved lifetime is unavailable for process handle `{}` in `{path}`",
                self.node(node).name
            )),
        }
    }

    pub(in super::super) fn collect_process_static_object(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<usize, String> {
        if let Some(index) = self.proc_process_static_objects.get(&(self.inst, node)) {
            return Ok(*index);
        }
        let is_process = matches!(
            self.kind(node),
            NodeKind::Var { ty }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
        );
        if !is_process {
            return Err(format!(
                "procedural process declaration `{}` in `{path}` has a non-process type",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Static {
            return Err(format!(
                "process handle `{}` in `{path}` is not static",
                self.node(node).name
            ));
        }
        let index = self.model.objects.len();
        self.model.objects.push(crate::sim::ir::IrObject {
            c_name: format!("O_P{}_L{}", self.inst.index(), node.index()),
            ty: crate::sim::ir::IrObjectType::Process,
            initial: None,
        });
        self.proc_process_static_objects
            .insert((self.inst, node), index);
        Ok(index)
    }

    pub(in super::super) fn proc_process_local_name(&self, node: NodeId) -> Option<&str> {
        self.proc_process_locals.get(&node).map(String::as_str)
    }

    /// Register a semaphore declared in a procedural body. Automatic
    /// semaphores live in the activation C frame; static declarations use a
    /// model object so repeated process activations share one semaphore.
    pub(in super::super) fn collect_semaphore_local(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<ChandleTarget, String> {
        if let Some(name) = self.proc_semaphore_locals.get(&node) {
            return Ok(ChandleTarget::Local(name.clone()));
        }
        if let Some(index) = self.proc_semaphore_static_objects.get(&(self.inst, node)) {
            return Ok(ChandleTarget::Object(*index));
        }
        let is_semaphore = matches!(
            self.kind(node),
            NodeKind::Var { ty }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
        );
        if !is_semaphore {
            return Err(format!(
                "procedural semaphore declaration `{}` has a non-semaphore type in `{path}`",
                self.node(node).name
            ));
        }
        match self.db.variable_lifetime(node) {
            VariableLifetime::Automatic => {
                let name = format!("_ls{}", node.index());
                self.proc_semaphore_locals.insert(node, name.clone());
                Ok(ChandleTarget::Local(name))
            }
            VariableLifetime::Static => {
                let index = self.collect_semaphore_static_object(path, node)?;
                if let Some(initializer) = self.db.var_initializer(node) {
                    if !matches!(
                        self.kind(initializer),
                        NodeKind::Expr(ExprKind::Constant {
                            const_type: ConstantType::Null,
                            ..
                        })
                    ) {
                        self.semaphore_initializers.push((
                            node,
                            index,
                            initializer,
                            path.to_owned(),
                        ));
                    }
                }
                Ok(ChandleTarget::Object(index))
            }
            VariableLifetime::Unavailable => Err(format!(
                "resolved lifetime is unavailable for semaphore `{}` in `{path}`",
                self.node(node).name
            )),
        }
    }

    pub(in super::super) fn collect_semaphore_static_object(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<usize, String> {
        if let Some(index) = self.proc_semaphore_static_objects.get(&(self.inst, node)) {
            return Ok(*index);
        }
        let is_semaphore = matches!(
            self.kind(node),
            NodeKind::Var { ty }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
        );
        if !is_semaphore {
            return Err(format!(
                "procedural semaphore declaration `{}` has a non-semaphore type in `{path}`",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Static {
            return Err(format!(
                "semaphore `{}` in `{path}` is not static",
                self.node(node).name
            ));
        }
        let index = self.model.objects.len();
        self.model.objects.push(crate::sim::ir::IrObject {
            c_name: format!("O_P{}_S{}", self.inst.index(), node.index()),
            ty: crate::sim::ir::IrObjectType::Semaphore,
            initial: None,
        });
        self.proc_semaphore_static_objects
            .insert((self.inst, node), index);
        Ok(index)
    }

    pub(in super::super) fn proc_semaphore_local_name(&self, node: NodeId) -> Option<&str> {
        self.proc_semaphore_locals.get(&node).map(String::as_str)
    }

    /// Resolve a procedural semaphore declaration through lexical begin/loop
    /// scopes, mirroring the process-handle resolver.
    pub(in super::super) fn lexical_proc_semaphore_decl(&self, reference: NodeId) -> Option<NodeId> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(
                        self.kind(**child),
                        NodeKind::Var { ty }
                            if ty.kind == "class"
                                && ty.type_name.as_deref() == Some("semaphore")
                    ) && self.node(**child).name == name
                }) {
                    return Some(*variable);
                }
            }
            let variable = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .find(|variable| {
                        matches!(
                            self.kind(**variable),
                            NodeKind::Var { ty }
                                if ty.kind == "class"
                                    && ty.type_name.as_deref() == Some("semaphore")
                        ) && self.node(**variable).name == name
                    })
                    .copied(),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .find(|variable| {
                        matches!(
                            self.kind(**variable),
                            NodeKind::Var { ty }
                                if ty.kind == "class"
                                    && ty.type_name.as_deref() == Some("semaphore")
                        ) && self.node(**variable).name == name
                    })
                    .copied(),
                _ => None,
            };
            if variable.is_some() {
                return variable;
            }
            parent = self.node(scope).parent;
        }
        None
    }

    /// Resolve a process declaration through begin/loop lexical scopes,
    /// including static declarations whose storage is model-backed.
    pub(in super::super) fn lexical_proc_process_decl(&self, reference: NodeId) -> Option<NodeId> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(self.kind(**child), NodeKind::Var { ty } if ty.kind == "class" && ty.type_name.as_deref() == Some("process"))
                        && self.node(**child).name == name
                }) {
                    return Some(*variable);
                }
            }
            if let NodeKind::Stmt(StmtKind::For { vars, .. }) = self.kind(scope) {
                if let Some(variable) = vars.iter().find(|variable| {
                    matches!(self.kind(**variable), NodeKind::Var { ty } if ty.kind == "class" && ty.type_name.as_deref() == Some("process"))
                        && self.node(**variable).name == name
                }) {
                    return Some(*variable);
                }
            }
            if let NodeKind::Stmt(StmtKind::Foreach { vars, .. }) = self.kind(scope) {
                if let Some(variable) = vars.iter().flatten().find(|variable| {
                    matches!(self.kind(**variable), NodeKind::Var { ty } if ty.kind == "class" && ty.type_name.as_deref() == Some("process"))
                        && self.node(**variable).name == name
                }) {
                    return Some(*variable);
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    pub(in super::super) fn is_foreach_iterator(&self, node: NodeId) -> bool {
        self.db.node_ids().any(|id| match self.kind(id) {
            NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => {
                vars.iter().flatten().any(|variable| *variable == node)
            }
            _ => false,
        })
    }

    /// Resolve a string loop iterator through its lexical statement scopes.
    /// This mirrors `lexical_proc_local` while keeping native-string storage
    /// out of packed expression paths.
    pub(in super::super) fn lexical_proc_string_local(&self, reference: NodeId) -> Option<(NodeId, &str)> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(self.kind(**child), NodeKind::Var { .. })
                        && self.node(**child).name == name
                }) {
                    if let Some(c_name) = self.proc_string_local_name(*variable) {
                        return Some((*variable, c_name));
                    }
                }
            }
            let variable = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .find(|variable| self.node(**variable).name == name)
                    .copied(),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .find(|variable| self.node(**variable).name == name)
                    .copied(),
                _ => None,
            };
            if let Some(variable) = variable {
                if let Some(c_name) = self.proc_string_local_name(variable) {
                    return Some((variable, c_name));
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    /// Resolve a process-local declaration in the current elaborated
    /// instance. Static declarations need the instance-qualified map because
    /// one owned declaration node can be instantiated more than once.
    pub(in super::super) fn proc_local_info(&self, node: NodeId) -> Option<&ProcLocalInfo> {
        match self.db.variable_lifetime(node) {
            VariableLifetime::Static => self.proc_local_instances.get(&(self.inst, node)),
            VariableLifetime::Automatic | VariableLifetime::Unavailable => {
                self.proc_locals.get(&node)
            }
        }
    }

    pub(in super::super) fn proc_local_target(&self, node: NodeId) -> Option<NodeId> {
        if let Some((variable, _)) = self.lexical_proc_local(node) {
            return self
                .proc_local_info(variable)
                .is_some_and(|info| info.static_signal.is_none())
                .then_some(variable);
        }
        if self.proc_local_is_shadowed(node) {
            return None;
        }
        match self.kind(node) {
            NodeKind::Var { .. }
                if self
                    .proc_local_info(node)
                    .is_some_and(|info| info.static_signal.is_none()) =>
            {
                Some(node)
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self
                .proc_local_info(*target)
                .is_some_and(|info| info.static_signal.is_none()) =>
            {
                Some(*target)
            }
            _ => None,
        }
    }

    pub(in super::super) fn lexical_proc_local(&self, reference: NodeId) -> Option<(NodeId, &ProcLocalInfo)> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(self.kind(**child), NodeKind::Var { .. })
                        && self.node(**child).name == name
                }) {
                    return self
                        .proc_local_info(*variable)
                        .map(|info| (*variable, info));
                }
            }
            let vars = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => {
                    return vars
                        .iter()
                        .find(|variable| self.node(**variable).name == name)
                        .and_then(|variable| {
                            self.proc_local_info(*variable)
                                .map(|info| (*variable, info))
                        });
                }
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => Some(vars.as_slice()),
                _ => None,
            };
            if let Some(variable) = vars.and_then(|vars| {
                vars.iter()
                    .flatten()
                    .find(|variable| self.node(**variable).name == name)
            }) {
                if let Some(info) = self.proc_local_info(*variable) {
                    return Some((*variable, info));
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    pub(in super::super) fn proc_local_is_shadowed(&self, reference: NodeId) -> bool {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin))
                && self.node(scope).children.iter().any(|child| {
                    matches!(self.kind(*child), NodeKind::Var { .. })
                        && self.node(*child).name == name
                })
            {
                return true;
            }
            let is_loop_var = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .any(|variable| self.node(*variable).name == name),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .any(|variable| self.node(*variable).name == name),
                _ => false,
            };
            if is_loop_var {
                return false;
            }
            parent = self.node(scope).parent;
        }
        false
    }

    pub(in super::super) fn nested_proc_local_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some((variable, info)) = self.lexical_proc_local(node) {
            return info.static_signal.is_none().then_some(variable);
        }
        if self.proc_local_is_shadowed(node) {
            return self
                .node(node)
                .children
                .iter()
                .find_map(|child| self.nested_proc_local_ref(*child));
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if self
                .proc_local_info(*target)
                .is_some_and(|info| info.static_signal.is_none())
            {
                return Some(*target);
            }
        }
        self.node(node)
            .children
            .iter()
            .find_map(|child| self.nested_proc_local_ref(*child))
    }

    /// Collect automatic values referenced by one evaluated event expression
    /// and place them in a private, copied activation frame. The callback may
    /// run after the issuing process suspends or is re-entered, so it must
    /// never refer directly to a lexical C local.
    pub(in super::super) fn event_context(
        &mut self,
        expression: NodeId,
    ) -> Result<Option<IrEventContext>, String> {
        fn visit(cg: &Codegen<'_>, node: NodeId, out: &mut HashSet<NodeId>) {
            let target = match cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => cg.lexical_proc_local(node).map(|(target, _)| target),
            };
            if let Some(target) = target {
                if cg
                    .capture_source(target)
                    .is_some_and(|source| source.info.static_signal.is_none())
                {
                    out.insert(target);
                }
            }
            for child in &cg.node(node).children {
                visit(cg, *child, out);
            }
        }

        let mut targets = HashSet::new();
        visit(self, expression, &mut targets);
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by_key(|target| target.index());
        if targets.is_empty() {
            return Ok(None);
        }

        let sources = targets
            .iter()
            .map(|target| {
                self.capture_source(*target).ok_or_else(|| {
                    format!(
                        "automatic declaration `{}` was not collected before event capture",
                        self.node(*target).name
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame = self.new_frame_id()?;
        let captures = sources
            .into_iter()
            .zip(targets)
            .filter_map(|(source, target)| {
                // Inlined const-ref formals can be substituted directly with
                // their signal/array actual. Such references already have a
                // stable dependency and must not manufacture a frame whose
                // initializer would be rendered outside the caller's formal
                // context. Only lexical C locals need a copied evaluator slot.
                let IrExprKind::LocalRead(local) = source.initial.kind() else {
                    return None;
                };
                Some((
                    self.declaration_identity(target),
                    local.clone(),
                    source.lifetime,
                    super::super::storage_kind(source.info.width),
                    source.initial,
                ))
            })
            .enumerate()
            .map(|(slot, (declaration, local, lifetime, kind, initial))| {
                Ok(IrEventCapture::new(
                    StorageRef::for_declaration(
                        frame,
                        slot as u32,
                        declaration?,
                        lifetime,
                        StorageOwnership::Owned,
                    )
                    .with_kind(kind),
                    local,
                    initial,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if captures.is_empty() {
            return Ok(None);
        }
        Ok(Some(IrEventContext::new(frame, captures)))
    }
}
