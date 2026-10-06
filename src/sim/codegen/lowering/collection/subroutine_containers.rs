//! Resizable containers in subroutine storage: formals, function results and
//! locals of functions and tasks (SV 7.5-7.10, 13.5).
//!
//! Automatic storage is an activation container created by
//! `IrContainerStmt::Declare` (locals and caller temporaries) or bound to the
//! caller-created storage of a container formal; static storage is one
//! model-global container per declaration and instance. Calls copy inputs in
//! and outputs back, so a callee never aliases caller storage.

use super::*;

impl Codegen<'_> {
    /// Resizable-container shape of a subroutine formal, result or local.
    pub(in super::super) fn subroutine_container_meta(
        &self,
        node: NodeId,
    ) -> Option<crate::core::db::ArrayMeta> {
        let meta = match self.kind(node) {
            NodeKind::FuncArg { .. } | NodeKind::FuncTask { .. } => {
                match self.db.subroutine_array_meta(node) {
                    Some(meta) => meta,
                    None => return self.fixed_view_meta(node),
                }
            }
            NodeKind::Var { .. } | NodeKind::Array { .. } => self.db.array_meta(node)?,
            NodeKind::NamedEvent => self.db.event_array_meta(node)?,
            _ => return None,
        };
        if matches!(meta.kind(), ArrayKind::Static) {
            return self.fixed_view_meta(node);
        }
        Some(meta.clone())
    }

    /// Shape of a one-dimensional fixed array of strings, native records or
    /// identity handles in subroutine storage: a dynamic container view of
    /// the declared size, like the same declaration in a module (SIM-007).
    fn fixed_view_meta(&self, node: NodeId) -> Option<crate::core::db::ArrayMeta> {
        let descriptor = self.query_descriptor(node)?;
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return None;
        };
        if !Self::is_fixed_handle_element(element) {
            return None;
        }
        Some(crate::core::db::ArrayMeta {
            kind: ArrayKind::Static,
            dims: dimensions.iter().copied().map(Some).collect(),
            init: self.db.array_meta(node).and_then(|meta| meta.initializer()),
            net_type: None,
        })
    }

    /// Whether a function result is a resizable container.
    pub(in super::super) fn container_return(&self, function: NodeId) -> bool {
        matches!(self.kind(function), NodeKind::FuncTask { ret: Some(_), .. })
            && self.subroutine_container_meta(function).is_some()
    }

    /// Callee storage of a container formal or result in one instance.
    pub(in super::super) fn container_formal_storage(
        &self,
        inst: NodeId,
        node: NodeId,
    ) -> Option<usize> {
        self.subroutine_containers.get(&(inst, node)).copied()
    }

    /// Whether `node` declares a container formal, result or local.
    pub(in super::super) fn is_subroutine_container(&self, node: NodeId) -> bool {
        self.subroutine_containers
            .keys()
            .any(|(_, declaration)| *declaration == node)
    }

    fn subroutine_container_nodes(&self, function: NodeId, output: &mut Vec<NodeId>) {
        output.extend(
            self.func_formals(function)
                .into_iter()
                .map(|(formal, _)| formal),
        );
        if self.container_return(function) {
            output.push(function);
        }
        if let Some(body) = self.func_body(function) {
            self.subroutine_container_locals(body, output);
        }
    }

    /// Whether a declaration reached by walking a subroutine body is one of
    /// its locals. Member declarations reached through selects are detached
    /// (no parent), and a keyed assignment pattern (`'{m: v}`) holds the
    /// record member's declaration under the pattern expression; neither is
    /// a local.
    pub(super) fn is_body_local(&self, node: NodeId) -> bool {
        self.node(node)
            .parent
            .is_some_and(|parent| !matches!(self.kind(parent), NodeKind::Expr(_)))
    }

    fn subroutine_container_locals(&self, node: NodeId, output: &mut Vec<NodeId>) {
        if matches!(
            self.kind(node),
            NodeKind::Var { .. } | NodeKind::Array { .. } | NodeKind::NamedEvent
        ) {
            if !self.is_body_local(node) {
                return;
            }
            output.push(node);
            return;
        }
        for child in &self.node(node).children {
            self.subroutine_container_locals(*child, output);
        }
    }

    /// Allocate container storage for the formals, result and locals of one
    /// subroutine instance. Static storage keeps declaration initializers in
    /// the model's static initialization; automatic locals initialize at
    /// their declaration statement.
    pub(in super::super) fn prepare_container_function(
        &mut self,
        inst: NodeId,
        function: NodeId,
        automatic: bool,
    ) -> Result<(), String> {
        let mut nodes = Vec::new();
        self.subroutine_container_nodes(function, &mut nodes);
        for node in nodes {
            let lifetime = match self.kind(node) {
                // A `ref` formal owns no storage: it aliases the caller's
                // container for the call (SIM-008), so even a static
                // subroutine binds it per call.
                NodeKind::FuncArg { direction, .. } => automatic || *direction == DbDirection::Ref,
                NodeKind::FuncTask { .. } => automatic,
                _ => self.db.variable_lifetime(node) == VariableLifetime::Automatic,
            };
            self.allocate_subroutine_container(inst, node, lifetime, function)?;
        }
        Ok(())
    }

    /// Storage for one container declaration of one instance: an activation
    /// container for automatic lifetime, otherwise a model-global container
    /// whose declaration initializer runs once with the static initializers.
    fn allocate_subroutine_container(
        &mut self,
        inst: NodeId,
        node: NodeId,
        automatic: bool,
        scope: NodeId,
    ) -> Result<Option<usize>, String> {
        if let Some(ir) = self.subroutine_containers.get(&(inst, node)) {
            return Ok(Some(*ir));
        }
        let Some(meta) = self.subroutine_container_meta(node) else {
            return Ok(None);
        };
        let name = self.node(node).name.clone();
        let element = if matches!(self.kind(node), NodeKind::NamedEvent) {
            if meta.dimensions().len() != 1 {
                return Err(format!(
                    "multidimensional event array `{name}` in subroutine storage is not supported"
                ));
            }
            IrContainerElement::Event
        } else {
            let descriptor = self
                .query_descriptor(node)
                .ok_or_else(|| format!("container `{name}` has no recursive type descriptor"))?;
            match &descriptor.shape {
                TypeShape::Container { element, .. } => lower_container_element(element)?,
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } if Self::is_fixed_handle_element(element) => {
                    if dimensions.len() != 1 {
                        return Err(format!(
                            "multidimensional fixed array `{name}` of {} elements in subroutine storage is not supported",
                            element.name
                        ));
                    }
                    lower_container_element(element)?
                }
                _ => {
                    return Err(format!(
                        "container `{name}` has a non-container type descriptor"
                    ))
                }
            }
        };
        let path = format!("llg_sub{}_{}", inst.index(), scope.index());
        let info = self.container_from_meta(&path, &name, node, &meta, element)?;
        let ir = info.ir;
        self.model.containers[ir].c_name = format!("S_llg_container_{ir}");
        if automatic {
            self.model.containers[ir].activation = true;
            // Automatic initializers run at each declaration entry.
            self.container_initializers
                .retain(|(declaration, _)| *declaration != node);
        }
        self.subroutine_containers.insert((inst, node), ir);
        Ok(Some(ir))
    }

    /// Storage of a container declared in a procedural block (or in a
    /// subroutine body lowered without `prepare_container_function`), bound
    /// for the statements that follow it.
    pub(in super::super) fn procedural_container(
        &mut self,
        declaration: NodeId,
        scope: NodeId,
    ) -> Result<Option<usize>, String> {
        let automatic = self.db.variable_lifetime(declaration) == VariableLifetime::Automatic;
        let Some(ir) =
            self.allocate_subroutine_container(self.inst, declaration, automatic, scope)?
        else {
            return Ok(None);
        };
        self.container_globals
            .insert(declaration, ContainerInfo { ir });
        Ok(Some(ir))
    }

    /// Bind the container declarations of the subroutine instance being
    /// lowered, so container references resolve to its storage.
    pub(in super::super) fn bind_container_function(&mut self, inst: NodeId, function: NodeId) {
        let mut nodes = Vec::new();
        self.subroutine_container_nodes(function, &mut nodes);
        for node in nodes {
            if let Some(ir) = self.subroutine_containers.get(&(inst, node)) {
                self.container_globals
                    .insert(node, ContainerInfo { ir: *ir });
            }
        }
    }

    /// A fresh activation container with the storage type of `like`, for a
    /// caller-side transfer. The caller declares it with `Declare`.
    pub(in super::super) fn container_temporary_like(&mut self, like: usize) -> usize {
        let ir = self.model.containers.len();
        let mut container = self.model.containers[like].clone();
        container.c_name = format!("S_llg_container_{ir}");
        container.activation = true;
        container.class_field = None;
        // A fixed-array view keeps its declared size, so an output formal
        // or result starts with default elements; other temporaries start
        // empty.
        self.model.containers.push(container);
        if let Some(range) = self.fixed_view_ranges.get(&like).copied() {
            self.fixed_view_ranges.insert(ir, range);
        }
        self.container_types_like.insert(ir, like);
        ir
    }

    /// Storage type of a container formal or result: every instance of one
    /// declaration shares its elaborated type.
    fn container_formal_type(&self, formal: NodeId) -> Result<usize, String> {
        self.subroutine_containers
            .iter()
            .filter(|((_, node), _)| *node == formal)
            .map(|(_, ir)| *ir)
            .min()
            .ok_or_else(|| {
                format!(
                    "container formal `{}` has no storage",
                    self.node(formal).name
                )
            })
    }

    /// Caller operand of a container formal. A whole container of the
    /// formal's storage type passes directly (the call copies it); an input
    /// of another form is built in a temporary before the call when
    /// `prelude` is available. Outputs require a container variable.
    pub(in super::super) fn container_call_argument(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
        prelude: Option<&mut Vec<IrStmt>>,
    ) -> Result<IrCallArg, String> {
        let like = self.container_formal_type(formal)?;
        let name = self.node(formal).name.clone();
        let direction = match self.kind(formal) {
            NodeKind::FuncArg { direction, .. } => *direction,
            _ => return Err(format!("container formal `{name}` is not an argument")),
        };
        if let Some(source) = self.container_of(self.p30_unwrap_cast(actual)) {
            if self.model.containers[source.ir].same_storage_type(&self.model.containers[like]) {
                return Ok(IrCallArg::Container(source.ir));
            }
        }
        if direction == DbDirection::Ref {
            return Err(format!(
                "ref actual of container formal `{name}` in `{path}` must be a container variable of the same type"
            ));
        }
        if direction != DbDirection::Input {
            return Err(format!(
                "output actual of container formal `{name}` in `{path}` must be a container variable of the same type"
            ));
        }
        if prelude.is_none() {
            if let Some(argument) = self.container_values_argument(path, like, actual)? {
                return Ok(argument);
            }
        }
        if let Some(prelude) = prelude {
            let temporary = self.container_temporary_like(like);
            prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                temporary,
            ))));
            prelude.push(self.lower_container_into(path, formal, temporary, actual)?);
            return Ok(IrCallArg::Container(temporary));
        }
        // Inside an assignment or system-task statement, an actual without
        // side effects may be built before the statement: evaluating it
        // early cannot be observed, even under a short-circuit.
        if self.container_call_prelude.is_some() && self.side_effect_free(actual) {
            let temporary = self.container_temporary_like(like);
            let build = self.lower_container_into(path, formal, temporary, actual)?;
            let prelude = self
                .container_call_prelude
                .as_mut()
                .ok_or("container call prelude closed while lowering its operand")?;
            prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                temporary,
            ))));
            prelude.push(build);
            return Ok(IrCallArg::Container(temporary));
        }
        Err(format!(
            "container argument for `{name}` in `{path}` must be a container variable of the formal's type unless the call is a whole statement; assign it to a variable first"
        ))
    }

    /// An assignment-pattern input of packed or real elements, evaluated
    /// element by element at the call itself (valid in any expression).
    fn container_values_argument(
        &mut self,
        path: &str,
        like: usize,
        actual: NodeId,
    ) -> Result<Option<IrCallArg>, String> {
        let storage = &self.model.containers[like];
        if matches!(storage.kind, IrContainerKind::Associative { .. })
            || !(storage.element.is_packed() || storage.element.is_real())
        {
            return Ok(None);
        }
        let Some(operands) =
            self.assignment_pattern_operands(path, self.p30_unwrap_cast(actual))?
        else {
            return Ok(None);
        };
        // A replicated operand would be evaluated once per element here, so
        // only patterns of distinct operands use this form.
        let mut seen = std::collections::HashSet::new();
        if !operands.iter().all(|operand| seen.insert(*operand)) {
            return Ok(None);
        }
        let real = storage.element.is_real();
        let temporary = self.container_temporary_like(like);
        let values = operands
            .into_iter()
            .map(|operand| {
                if real {
                    self.lower_expr(path, operand)
                } else {
                    self.lower_container_value(path, temporary, operand)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(IrCallArg::ContainerValues {
            container: temporary,
            values,
        }))
    }

    /// Store a call to a container-result function directly into `dst`: the
    /// call gives the callee fresh result storage and copies it back.
    pub(in super::super) fn lower_container_result_into(
        &mut self,
        path: &str,
        rhs: NodeId,
        dst: usize,
    ) -> Result<Option<IrStmt>, String> {
        let call = self.p30_unwrap_cast(rhs);
        let (name, callee) = match self.kind(call) {
            NodeKind::FuncCall { name, callee, .. } | NodeKind::MethodCall { name, callee, .. } => {
                (name.clone(), *callee)
            }
            _ => return Ok(None),
        };
        let function = match self.kind(call) {
            NodeKind::FuncCall { .. } => {
                self.resolve_callee_env(self.inst, &name, false, callee)?.0
            }
            _ => match callee {
                Some(function) => function,
                None => return Ok(None),
            },
        };
        if !self.container_return(function) {
            return Ok(None);
        }
        let result = self.container_formal_type(function)?;
        if !self.model.containers[dst].same_storage_type(&self.model.containers[result]) {
            return Err(format!(
                "result of `{name}` in `{path}` must be assigned to a container of the same type"
            ));
        }
        let saved = std::mem::replace(&mut self.container_result_call, true);
        let expression = self.lower_func_call_expr(path, call, &name, callee);
        self.container_result_call = saved;
        let IrExprKind::CallFn(expression) = expression?.kind else {
            return Err("container result call did not lower to a typed call".into());
        };
        let mut args = expression.args;
        let outputs = self.model.funcs[expression.f]
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .count();
        args.insert(outputs - 1, IrCallArg::Container(dst));
        let mut lowered = IrCall::new(expression.f, args, expression.depth, Vec::new(), Vec::new());
        lowered.receiver = expression.receiver;
        lowered.virtual_dispatch = expression.virtual_dispatch;
        lowered.virtual_call = expression.virtual_call;
        Ok(Some(IrStmt::Call(Box::new(lowered))))
    }
}
