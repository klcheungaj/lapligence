//! Call contracts.

use super::*;

#[cfg(test)]
mod tests;

/// How calls to a subroutine reach its body.
///
/// A subroutine is called through its typed `IrFunc` unless an output, inout
/// or ref event formal, or an event control reading a formal or local other
/// than an input event formal or a `ref` formal, needs the caller's
/// environment (`inline_only`: the call site expands the body). A `ref`
/// formal read by an event control (directly or through a callee it is
/// forwarded to) is `static_refs`: its evaluator's dependencies are those of
/// the actual, which a shared body cannot know, so each distinct static
/// actual gets its own specialization of the task.
#[derive(Clone, Debug, Default)]
pub(in super::super) struct CallShape {
    pub(in super::super) inline_only: bool,
    /// Formal indices that a specialization binds to a whole module signal.
    pub(in super::super) static_refs: Vec<usize>,
}

/// Where an evaluated event expression is computed.
///
/// `Callback` expressions are re-evaluated by the runtime inside the write
/// that changed one of their dependencies, so their helpers must be
/// read-only. `Process` expressions are legal zero-time helpers whose
/// effects are visible (writes to external or persistent storage, static
/// result state) or whose formals use descriptor transport: the waiting
/// process evaluates them itself, at arm time and after each dependency
/// change, with ordinary call semantics. The `String` keeps the read-only
/// rejection reason for contexts that only have the callback form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum EventEvaluation {
    Callback,
    Process(String),
}

/// How a `$monitor`/`$strobe` argument is evaluated in the Postponed region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum PostponedEvaluation {
    /// Read-only callback (private inlined helpers only).
    Callback,
    /// Ordinary calls whose only stores are to the helpers' own storage;
    /// the runtime applies them without publication.
    PrivateEffects,
}

impl<'a> Codegen<'a> {
    pub(in super::super) fn check_event_expression_effects(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<(), String> {
        match self.classify_event_expression(expression, scope_path)? {
            EventEvaluation::Callback => Ok(()),
            EventEvaluation::Process(reason) => Err(reason),
        }
    }

    /// Classify an evaluated event expression. Forms that are not legal
    /// zero-time read contexts at all (tasks, timing, unsummarized system or
    /// method calls, writable formals) still reject; effects that only rule
    /// out the read-only callback select process evaluation.
    pub(in super::super) fn classify_event_expression(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<EventEvaluation, String> {
        Ok(
            match self.event_expression_effects(expression, scope_path)?.0 {
                Some(reason) => EventEvaluation::Process(reason),
                None => EventEvaluation::Callback,
            },
        )
    }

    /// Classify a `$monitor`/`$strobe` argument, which the runtime evaluates
    /// in the Postponed region. SV 4.4.2.9 makes writing any net or variable
    /// there illegal, so a helper that writes storage outside itself is
    /// rejected. A helper whose only effects are its own static state
    /// (formals, locals, result) or descriptor transport is evaluated with
    /// ordinary call semantics; those private stores publish no event.
    pub(in super::super) fn classify_postponed_expression(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<PostponedEvaluation, String> {
        // Forms without a zero-time effect summary (system and method calls,
        // timing) keep the callback path and its own diagnostics.
        let Ok((process, external)) = self.event_expression_effects(expression, scope_path) else {
            return Ok(PostponedEvaluation::Callback);
        };
        if let Some(reason) = external {
            return Err(format!(
                "{reason}: `$monitor`/`$strobe` arguments are evaluated in the read-only Postponed region (IEEE 1800-2009 4.4.2.9)"
            ));
        }
        Ok(if process.is_some() {
            PostponedEvaluation::PrivateEffects
        } else {
            PostponedEvaluation::Callback
        })
    }

    /// `(process, external)`: why the expression cannot be a read-only
    /// callback, and why it writes storage its helpers do not own.
    fn event_expression_effects(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<(Option<String>, Option<String>), String> {
        let mut visited = HashSet::new();
        let mut process = None;
        let mut external = None;
        self.check_event_node(
            expression,
            scope_path,
            &mut visited,
            None,
            &mut process,
            &mut external,
        )?;
        Ok((process, external))
    }

    fn check_event_node(
        &self,
        node: NodeId,
        scope_path: &str,
        visited_functions: &mut HashSet<NodeId>,
        function: Option<NodeId>,
        process: &mut Option<String>,
        external: &mut Option<String>,
    ) -> Result<(), String> {
        let rejected = |reason: &str| {
            Err(format!(
                "function calls in evaluated event controls are not supported in `{scope_path}`: {reason}"
            ))
        };
        let needs_process = |process: &mut Option<String>, reason: &str| {
            process.get_or_insert_with(|| {
                format!(
                    "function calls in evaluated event controls are not supported in `{scope_path}`: {reason}"
                )
            });
        };
        match self.kind(node) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                if *is_task {
                    return rejected("task calls are not read-only");
                }
                let (ft, callee_inst) = self
                    .resolve_callee_env(self.inst, name, false, *callee)
                    .map_err(|_| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: cannot resolve `{name}`"
                        )
                    })?;
                let (_, _, formals) = self.func_info(ft, callee_inst).map_err(|error| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: {error}"
                    )
                })?;
                for (formal, is_out) in formals {
                    if is_out {
                        return rejected("output/inout formals are not read-only");
                    }
                    if matches!(
                        self.kind(formal),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            const_ref: false,
                            ..
                        }
                    ) {
                        return rejected("non-const ref formals are not read-only");
                    }
                }
                if visited_functions.insert(ft) {
                    let body = self.func_body(ft).ok_or_else(|| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: function `{name}` has no body"
                        )
                    })?;
                    if matches!(
                        self.kind(ft),
                        NodeKind::FuncTask {
                            automatic: false,
                            ret: Some(_),
                            ..
                        }
                    ) && !self.static_return_is_callback_independent(body, ft)
                    {
                        needs_process(
                            process,
                            "static function return is read or is not assigned on every path",
                        );
                    }
                    if self.nonflatten_function(ft)
                        || self.record_return(ft)
                        || self.func_formals(ft).iter().any(|(formal, _)| {
                            self.nonflatten_function(*formal) || self.record_declaration(*formal)
                        })
                    {
                        needs_process(
                            process,
                            "descriptor-transported fixed-array formals or result",
                        );
                    }
                    if self.real_array_return(ft)
                        || self
                            .func_formals(ft)
                            .iter()
                            .any(|(formal, _)| self.real_array_shape(*formal).is_some())
                    {
                        needs_process(process, "real fixed-array formals or result");
                    }
                    self.check_event_node(
                        body,
                        scope_path,
                        visited_functions,
                        Some(ft),
                        process,
                        external,
                    )?;
                }
            }
            NodeKind::SysCall { name } => {
                return rejected(&format!("system call `{name}` has no pure effect summary"));
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if matches!(name.as_str(), "sum" | "product" | "and" | "or" | "xor") => {
                let fixed_array = self.query_descriptor(*receiver).is_some_and(|descriptor| {
                    matches!(descriptor.shape, TypeShape::FixedArray { .. })
                });
                if !fixed_array {
                    return rejected(&format!(
                        "fixed-array reduction method `{name}` has no pure effect summary"
                    ));
                }
            }
            NodeKind::MethodCall { name, .. } => {
                return rejected(&format!("method call `{name}` has no pure effect summary"));
            }
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::CycleDelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. }
                | StmtKind::EventTrigger { .. }
                | StmtKind::Force { .. }
                | StmtKind::Release { .. }
                | StmtKind::ProcContAssign { .. }
                | StmtKind::Fork { .. }
                | StmtKind::WaitFork
                | StmtKind::DisableFork,
            ) => return rejected("timing, event, or scheduler effects are not allowed"),
            NodeKind::Stmt(StmtKind::Assign { .. }) => {
                let lhs = self.node(node).children.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    needs_process(
                        process,
                        "function body writes external or persistent storage",
                    );
                    if !self.event_owned_write(function, lhs) {
                        external.get_or_insert_with(|| {
                            format!("a helper in `{scope_path}` writes storage it does not own")
                        });
                    }
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                assignment,
                operands,
                ..
            }) if *assignment
                || matches!(
                    op,
                    Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement
                        | Operation::Assignment
                ) =>
            {
                let lhs = operands.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment expression"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    needs_process(
                        process,
                        "function body writes external or persistent storage",
                    );
                    if !self.event_owned_write(function, lhs) {
                        external.get_or_insert_with(|| {
                            format!("a helper in `{scope_path}` writes storage it does not own")
                        });
                    }
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.check_event_node(
                *child,
                scope_path,
                visited_functions,
                function,
                process,
                external,
            )?;
        }
        Ok(())
    }

    /// Static result storage is persistent. A callback may use a private
    /// result cell only when source analysis proves it is never read and a
    /// result value is established on every normal or explicit return path.
    pub(super) fn static_return_is_callback_independent(
        &self,
        body: NodeId,
        function: NodeId,
    ) -> bool {
        !self.function_reads_return(body, function)
            && self.function_establishes_return_on_every_path(body, function)
    }

    /// A static function result may be evaluated in a private callback frame
    /// only if that frame cannot observe a previous invocation's result.
    /// `Access::Write` follows an lvalue's storage path without counting the
    /// target itself as a read; read-modify-write operations use `ReadWrite`.
    fn function_reads_return(&self, root: NodeId, function: NodeId) -> bool {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Access {
            Read,
            Write,
            ReadWrite,
        }

        fn visit(cg: &Codegen<'_>, node: NodeId, function: NodeId, access: Access) -> bool {
            match cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if cg.canonical_func_target(*target).unwrap_or(*target) == function => {
                    return access != Access::Write;
                }
                NodeKind::Stmt(StmtKind::Assign { op, .. }) => {
                    if let Some(lhs) = cg.node(node).children.first() {
                        let access = if matches!(op, Operation::Assignment) {
                            Access::Write
                        } else {
                            Access::ReadWrite
                        };
                        if visit(cg, *lhs, function, access) {
                            return true;
                        }
                    }
                    return cg
                        .node(node)
                        .children
                        .iter()
                        .skip(1)
                        .any(|child| visit(cg, *child, function, Access::Read));
                }
                NodeKind::Expr(ExprKind::Operation {
                    op,
                    assignment,
                    operands,
                    ..
                }) if *assignment
                    || matches!(
                        op,
                        Operation::Assignment
                            | Operation::PostIncrement
                            | Operation::PreIncrement
                            | Operation::PostDecrement
                            | Operation::PreDecrement
                    ) =>
                {
                    if let Some(lhs) = operands.first() {
                        let reads_lhs = *assignment && !matches!(op, Operation::Assignment)
                            || matches!(
                                op,
                                Operation::PostIncrement
                                    | Operation::PreIncrement
                                    | Operation::PostDecrement
                                    | Operation::PreDecrement
                            );
                        if visit(
                            cg,
                            *lhs,
                            function,
                            if reads_lhs {
                                Access::ReadWrite
                            } else {
                                Access::Write
                            },
                        ) {
                            return true;
                        }
                    }
                    return operands
                        .iter()
                        .skip(1)
                        .any(|operand| visit(cg, *operand, function, Access::Read));
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) if access != Access::Read => {
                    return visit(cg, *base, function, access)
                        || visit(cg, *index, function, Access::Read);
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right })
                    if access != Access::Read =>
                {
                    return visit(cg, *base, function, access)
                        || visit(cg, *left, function, Access::Read)
                        || visit(cg, *right, function, Access::Read);
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    ..
                }) if access != Access::Read => {
                    return visit(cg, *base, function, access)
                        || visit(cg, *base_expr, function, Access::Read)
                        || visit(cg, *width_expr, function, Access::Read);
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                    if access != Access::Read =>
                {
                    return visit(cg, *base, function, access)
                        || indices
                            .iter()
                            .any(|index| visit(cg, *index, function, Access::Read));
                }
                _ => {}
            }
            cg.node(node)
                .children
                .iter()
                .any(|child| visit(cg, *child, function, Access::Read))
        }

        visit(self, root, function, Access::Read)
    }

    /// Whether every leaf of an assignment target is storage the called
    /// function owns: its result, or a formal or local declared inside it,
    /// static or automatic.
    fn event_owned_write(&self, function: Option<NodeId>, lhs: NodeId) -> bool {
        let Some(function) = function else {
            return false;
        };
        let pattern = self.p30_unwrap_cast(lhs);
        match self.kind(pattern) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => {
                return !operands.is_empty()
                    && operands
                        .iter()
                        .all(|operand| self.event_owned_write(Some(function), *operand));
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                operands,
                ..
            }) => {
                return !operands.is_empty()
                    && operands.iter().all(|operand| {
                        self.p30_pattern_lvalue_operand("callback", *operand)
                            .is_ok_and(|target| self.event_owned_write(Some(function), target))
                    });
            }
            _ => {}
        }
        let Some(target) = self.assignment_storage_root(lhs) else {
            return false;
        };
        let target = self.canonical_func_target(target).unwrap_or(target);
        target == function
            || (self.node_is_within(target, function)
                && matches!(
                    self.kind(target),
                    NodeKind::FuncArg {
                        direction: DbDirection::Input,
                        ..
                    } | NodeKind::Var { .. }
                        | NodeKind::Array { .. }
                ))
    }

    fn event_local_write_allowed(&self, function: Option<NodeId>, lhs: NodeId) -> bool {
        let Some(function) = function else {
            return false;
        };
        // A composite target is harmless only when every actual leaf is
        // private. Do not turn a concat containing one global into a local
        // merely because its first operand is local.
        let pattern = self.p30_unwrap_cast(lhs);
        match self.kind(pattern) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => {
                return !operands.is_empty()
                    && operands
                        .iter()
                        .all(|operand| self.event_local_write_allowed(Some(function), *operand));
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                operands,
                ..
            }) => {
                return !operands.is_empty()
                    && operands.iter().all(|operand| {
                        self.p30_pattern_lvalue_operand("callback", *operand)
                            .is_ok_and(|target| {
                                self.event_local_write_allowed(Some(function), target)
                            })
                    });
            }
            _ => {}
        }
        let target = self.assignment_storage_root(lhs);
        let Some(target) = target else {
            return false;
        };
        let target = self.canonical_func_target(target).unwrap_or(target);
        if target == function {
            return true;
        }
        if !self.node_is_within(target, function) {
            return false;
        }
        match self.kind(target) {
            NodeKind::FuncArg {
                direction: DbDirection::Input,
                ..
            }
            | NodeKind::Var { .. }
            | NodeKind::Array { .. } => {
                self.db.variable_lifetime(target) != VariableLifetime::Static
            }
            _ => false,
        }
    }

    /// Resolve a call site's callee and its owning elaborated environment.
    /// The captured semantic target is authoritative: its parent chain gives
    /// the concrete module/interface instance or the single shared package.
    /// Name lookup remains only for local calls whose frontend snapshot did
    /// not retain a callee edge.
    pub(in super::super) fn resolve_callee_env(
        &self,
        inst: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<(NodeId, NodeId), String> {
        if let Some(ft) = callee {
            if !matches!(self.kind(ft), NodeKind::FuncTask { is_task: t, .. } if *t == is_task) {
                return Err(format!(
                    "callee `{name}` has an incompatible function/task kind"
                ));
            }
            let environment = self.callable_environment(ft).ok_or_else(|| {
                format!(
                    "callee `{name}` has no elaborated module, interface, or package environment"
                )
            })?;
            return Ok((ft, environment));
        }
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task: t, .. } = self.kind(*c) {
                if *t == is_task && self.node(*c).name == name {
                    return Ok((*c, inst));
                }
            }
        }
        Err(format!(
            "cannot resolve callee `{name}` in `{}`",
            self.node(inst).name
        ))
    }

    /// Slang records a mailbox constructor's anonymous built-in callee as a
    /// normal function call. It is consumed by mailbox lowering and must not
    /// be resolved as a user function while collecting process contracts or
    /// signal dependencies.
    pub(in super::super) fn is_mailbox_constructor_call(&self, node: NodeId) -> bool {
        let NodeKind::FuncCall {
            name,
            callee: Some(_),
            ..
        } = self.kind(node)
        else {
            return false;
        };
        if name != "new" {
            return false;
        }
        let Some(parent) = self.node(node).parent else {
            return false;
        };
        matches!(self.kind(parent), NodeKind::Expr(ExprKind::NewClass { .. }))
            && self
                .db
                .type_descriptor(parent)
                .is_some_and(|descriptor| descriptor.name.starts_with("mailbox#("))
    }

    /// The concrete environment that owns one captured subroutine clone.
    /// This is structural and never reconstructed from a display name, so
    /// sibling instances and package users cannot alias.
    pub(super) fn callable_environment(&self, ft: NodeId) -> Option<NodeId> {
        let mut current = self.node(ft).parent;
        while let Some(id) = current {
            if matches!(self.kind(id), NodeKind::ModuleInst { .. })
                || self.is_runtime_environment(id)
                || matches!(self.kind(id), NodeKind::ClassDef)
            {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Whether a task's execution can suspend: its body (transitively over
    /// called tasks) contains a delay, event control or wait statement.
    /// Delay-bearing tasks are inlined at their call sites; the others become
    /// plain C functions.
    pub(in super::super) fn task_has_wait(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_wait_inner(ft, inst, &mut seen)
    }

    /// Whether the retained definition of a subroutine is only a template:
    /// calls either expand it (`lower_task_inline`) or call a specialization
    /// of it that binds its static `ref` formals (see [`CallShape`]).
    pub(in super::super) fn subroutine_requires_inline(&self, ft: NodeId, inst: NodeId) -> bool {
        let shape = self.call_shape(ft, inst);
        shape.inline_only || !shape.static_refs.is_empty()
    }

    /// How calls to a subroutine reach its body.
    pub(in super::super) fn call_shape(&self, ft: NodeId, inst: NodeId) -> CallShape {
        self.call_shape_of(ft, inst, &mut Vec::new())
    }

    fn call_shape_of(&self, ft: NodeId, inst: NodeId, stack: &mut Vec<NodeId>) -> CallShape {
        let mut shape = CallShape::default();
        // A recursive call adds nothing its outer activation does not.
        if stack.contains(&ft) {
            return shape;
        }
        let formals = self.func_formals(ft);
        if formals.iter().any(|(formal, _)| {
            matches!(
                self.kind(*formal),
                NodeKind::FuncArg { ty, direction, .. }
                    if ty.kind == "event" && *direction != DbDirection::Input
            )
        }) {
            shape.inline_only = true;
        }
        if let Some(body) = self.func_body(ft) {
            stack.push(ft);
            self.call_shape_walk(body, inst, &formals, stack, &mut shape);
            stack.pop();
        }
        shape.static_refs.sort_unstable();
        shape.static_refs.dedup();
        shape
    }

    fn call_shape_walk(
        &self,
        node: NodeId,
        inst: NodeId,
        formals: &[(NodeId, bool)],
        stack: &mut Vec<NodeId>,
        shape: &mut CallShape,
    ) {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit,
                body,
            }) => {
                let mut roots = Vec::new();
                for spec in specs {
                    self.event_spec_expressions(spec, &mut roots);
                }
                if *implicit {
                    roots.extend(body);
                }
                for root in roots {
                    self.note_static_reads(root, formals, shape);
                }
            }
            NodeKind::FuncCall {
                name,
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((function, owner)) = self.resolve_callee_env(inst, name, true, *callee) {
                    let inner = self.call_shape_of(function, owner, stack);
                    shape.inline_only |= inner.inline_only;
                    let arguments = self.call_argument_nodes(node);
                    for index in inner.static_refs {
                        // The callee's clone binds this actual, so it must be a
                        // formal of this task or a module-level name.
                        if let Some(actual) = arguments.get(index) {
                            if !self.note_forwarded_ref(*actual, formals, shape) {
                                shape.inline_only = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.call_shape_walk(*child, inst, formals, stack, shape);
        }
    }

    /// Expression roots an event spec evaluates.
    fn event_spec_expressions(&self, spec: &EventSpec, out: &mut Vec<NodeId>) {
        match spec {
            EventSpec::Qualified { event, condition } => {
                self.event_spec_expressions(event, out);
                out.push(*condition);
            }
            EventSpec::Named(event) => out.push(*event),
            EventSpec::AnyChange { sig } | EventSpec::Edge { sig, .. } => out.push(*sig),
        }
    }

    /// Classify an event control's reads of subroutine-scoped storage. A `ref`
    /// formal of the task itself is bound statically by a specialization;
    /// input event formals are typed parameters; any other formal or local
    /// forces the call-site expansion, whose evaluator context captures it.
    fn note_static_reads(&self, node: NodeId, formals: &[(NodeId, bool)], shape: &mut CallShape) {
        let mut visit_target = |target: NodeId| {
            if let Some(index) = formals.iter().position(|(formal, _)| *formal == target) {
                match self.kind(target) {
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ty,
                        ..
                    } if ty.kind != "string" && !is_handle_kind(&ty.kind) => {
                        shape.static_refs.push(index);
                    }
                    NodeKind::FuncArg {
                        direction: DbDirection::Input,
                        ty,
                        ..
                    } if ty.kind == "event" => {}
                    _ => shape.inline_only = true,
                }
            } else if self.is_subroutine_scoped(target) {
                shape.inline_only = true;
            }
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => visit_target(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter()
                    .flatten()
                    .for_each(|target| visit_target(*target));
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.note_static_reads(*child, formals, shape);
        }
    }

    /// Record a `ref` formal forwarded as the actual of a statically bound
    /// formal. Only a whole formal or module-level name binds statically.
    fn note_forwarded_ref(
        &self,
        actual: NodeId,
        formals: &[(NodeId, bool)],
        shape: &mut CallShape,
    ) -> bool {
        let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(actual)
        else {
            return false;
        };
        if let Some(index) = formals.iter().position(|(formal, _)| formal == target) {
            if matches!(
                self.kind(*target),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            ) {
                shape.static_refs.push(index);
                return true;
            }
            return false;
        }
        !self.is_subroutine_scoped(*target)
    }

    /// Whether a declaration lives in a subroutine activation (a formal or a
    /// local), as opposed to module, package or compilation-unit storage.
    fn is_subroutine_scoped(&self, target: NodeId) -> bool {
        matches!(
            self.kind(target),
            NodeKind::FuncArg { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
        ) && self.enclosing_func_task(target).is_some()
    }

    /// Whether a task can cancel its activation through a named `disable`.
    /// Its call sites keep the activation visible to the caller's
    /// cancellation check, so output/inout copy-out is skipped.
    pub(in super::super) fn task_has_disable(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_disable_inner(ft, inst, &mut seen)
    }

    /// Whether a task declaration is the target of an explicit `disable`.
    ///
    /// If an external disable can name the task, its call sites wrap the call
    /// in the declaration's activation so the cancellation outlives the
    /// callee's own scope until copy-out is skipped. This is deliberately a
    /// declaration-level check: every invocation shares the same runtime
    /// activation identity.
    pub(in super::super) fn task_is_disable_target(&self, ft: NodeId) -> bool {
        self.db.node_ids().any(|node| {
            matches!(
                self.kind(node),
                NodeKind::Stmt(StmtKind::Disable {
                    target: Some(target)
                }) if *target == ft
            )
        })
    }

    /// Whether an NBA in `node` targets subroutine storage which does not
    /// outlive the generated C call. Static formals, locals and return
    /// variables use persistent storage; every automatic formal/local is
    /// call-stack storage.
    pub(in super::super) fn node_has_stack_backed_subroutine_nba(
        &self,
        node: NodeId,
        subroutine: NodeId,
        automatic: bool,
    ) -> bool {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // IEEE 1800-2009 §12.7 restricts for-initialization to variable
            // assignments and for-step assignments to operator assignments,
            // increment/decrement expressions, or function calls. Inline
            // declaration initializers are emitted as blocking assignments;
            // lower_for deliberately emits those assignments as blocking.
            // Only the loop body can therefore contain an NBA owned by this
            // subroutine.
            return self.node_has_stack_backed_subroutine_nba(*body, subroutine, automatic);
        }
        if let NodeKind::Stmt(StmtKind::Assign {
            blocking: false, ..
        }) = self.kind(node)
        {
            if let Some(lhs) = self.node(node).children.first().copied() {
                let target = self.assignment_storage_root(lhs);
                let declaration = target.or_else(|| {
                    let name = self
                        .node(lhs)
                        .name
                        .split_once('[')
                        .map_or(self.node(lhs).name.as_str(), |(name, _)| name);
                    self.subroutine_storage_named(subroutine, name)
                });
                let mut current = declaration;
                while let Some(target) = current {
                    if target == subroutine {
                        return declaration.is_some_and(|declaration| {
                            match self.kind(declaration) {
                                NodeKind::FuncArg { .. } => automatic,
                                NodeKind::Var { .. } | NodeKind::Array { .. } => {
                                    self.db.variable_lifetime(declaration)
                                        == VariableLifetime::Automatic
                                }
                                _ => false,
                            }
                        });
                    }
                    current = self.node(target).parent;
                }
            }
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.node_has_stack_backed_subroutine_nba(*child, subroutine, automatic))
    }

    fn subroutine_storage_named(&self, node: NodeId, name: &str) -> Option<NodeId> {
        for child in &self.node(node).children {
            if matches!(
                self.kind(*child),
                NodeKind::FuncArg { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
            ) && self.node(*child).name == name
            {
                return Some(*child);
            }
            if let Some(found) = self.subroutine_storage_named(*child, name) {
                return Some(found);
            }
        }
        None
    }

    pub(super) fn collect_subroutine_decl_initializers(
        &self,
        node: NodeId,
        initializers: &mut HashMap<NodeId, NodeId>,
    ) {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // A for initializer can also expose a direct Var LHS, but it must
            // execute on loop entry rather than initialize function storage.
            self.collect_subroutine_decl_initializers(*body, initializers);
            return;
        }
        if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Assign { .. })) {
            if let [lhs, rhs, ..] = self.node(node).children.as_slice() {
                if matches!(self.kind(*lhs), NodeKind::Var { .. }) {
                    initializers.insert(*lhs, *rhs);
                    return;
                }
            }
        }
        for child in &self.node(node).children {
            self.collect_subroutine_decl_initializers(*child, initializers);
        }
    }

    fn task_has_wait_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_wait(body, inst, seen)
    }

    fn task_has_disable_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_disable(body, inst, seen)
    }

    fn node_has_disable(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Disable { .. }) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_disable_inner(ft, callee_inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_disable(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_disable(*c, inst, seen)),
        }
    }

    fn node_has_wait(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::CycleDelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. },
            ) => true,
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind: ConcurrentAssertionKind::Expect,
                ..
            }) => true,
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if matches!(name.as_str(), "suspend" | "await")
                && self.is_process_expr(&self.instance_path_of(inst), *receiver) =>
            {
                true
            }
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_wait_inner(ft, callee_inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_wait(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_wait(*c, inst, seen)),
        }
    }

    /// Resolve a function/task body write target (output/inout formal, local
    /// or return variable) to an LHS.  Tries `node` first (locals and the
    /// return var are indexed), then `name`.
    pub(super) fn func_write_target(&self, node: NodeId, name: &str) -> Option<Lhs> {
        let f = self.func.as_ref()?;
        let node = self.canonical_func_target(node).unwrap_or(node);
        if let Some(lhs) = f.arg_lhs.get(&node) {
            return Some(lhs.clone());
        }
        if f.const_refs.contains(&node) {
            return None;
        }
        if let Some(info) = f.persistent.get(&node) {
            return Some(Lhs::Whole(info.clone()));
        }
        if let Some(addr) = f.arg_write.get(&node) {
            if let Some(am) = f.arg_read.get(&node) {
                return Some(Lhs::WholeRef {
                    addr: addr.clone(),
                    width: am.width,
                    signed: am.signed,
                    two_state: am.two_state,
                    shortreal: matches!(
                        self.kind(node),
                        NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                    ),
                });
            }
        }
        if let Some((cname, w, s, two_state, shortreal)) = f.locals.get(&node) {
            return Some(Lhs::WholeRef {
                addr: format!("&{cname}"),
                width: *w,
                signed: *s,
                two_state: *two_state,
                shortreal: *shortreal,
            });
        }
        if f.ret_node == Some(node) {
            if let Some(r) = &f.ret {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        for io in sorted_node_ids(&f.arg_write) {
            let addr = &f.arg_write[&io];
            if self.node(io).name == name {
                if let Some(info) = f.persistent.get(&io) {
                    return Some(Lhs::Whole(info.clone()));
                }
                if let Some(am) = f.arg_read.get(&io) {
                    return Some(Lhs::WholeRef {
                        addr: addr.clone(),
                        width: am.width,
                        signed: am.signed,
                        two_state: am.two_state,
                        shortreal: matches!(
                            self.kind(io),
                            NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                        ),
                    });
                }
            }
        }
        for io in sorted_node_ids(&f.arg_lhs) {
            if self.node(io).name == name {
                return Some(f.arg_lhs[&io].clone());
            }
        }
        for nid in sorted_node_ids(&f.locals) {
            let (cname, w, s, two_state, shortreal) = &f.locals[&nid];
            if self.node(nid).name == name {
                if let Some(storage) = f.persistent.get(&nid) {
                    return Some(Lhs::Whole(storage.clone()));
                }
                return Some(Lhs::WholeRef {
                    addr: format!("&{cname}"),
                    width: *w,
                    signed: *s,
                    two_state: *two_state,
                    shortreal: *shortreal,
                });
            }
        }
        if let Some(r) = &f.ret {
            if r.node.map(|n| self.node(n).name == name).unwrap_or(false) {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        None
    }

    pub(super) fn is_const_ref_target(&self, node: NodeId, name: &str) -> bool {
        let Some(func) = self.func.as_ref() else {
            return false;
        };
        let node = self.canonical_func_target(node).unwrap_or(node);
        func.const_refs.contains(&node)
            || func
                .const_refs
                .iter()
                .any(|formal| self.node(*formal).name == name)
    }

    pub(in super::super) fn subroutine_auto_target(&self, node: NodeId) -> bool {
        self.subroutine_auto_ref(node).is_some()
    }

    /// Return the canonical automatic subroutine storage referenced by a
    /// target or expression node.  Force evaluators are emitted outside the
    /// activation that issued them, so stack-backed formals and locals may
    /// not be captured; persistent static subroutine storage remains valid.
    pub(in super::super) fn subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        let function = self.func.as_ref()?;
        if !self.function_is_automatic(function) {
            return None;
        }
        let target = self.assignment_storage_root(node)?;
        let target = self.canonical_func_target(target).unwrap_or(target);
        if function.persistent.contains_key(&target) {
            return None;
        }
        (function.locals.contains_key(&target)
            || function.arg_read.contains_key(&target)
            || function.arg_write.contains_key(&target)
            || function.ret_node == Some(target))
        .then_some(target)
    }

    /// Return the storage declaration before any packed members/selectors.
    /// Walking to the root keeps callback purity and automatic-lifetime checks
    /// independent of how many member or indexing nodes the frontend emits.
    fn assignment_storage_root(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Var { .. }
            | NodeKind::Array { .. }
            | NodeKind::FuncArg { .. }
            | NodeKind::FuncTask { .. } => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.assignment_storage_root(*base),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().flatten().copied().find(|target| {
                    matches!(
                        self.kind(*target),
                        NodeKind::Var { .. }
                            | NodeKind::Array { .. }
                            | NodeKind::FuncArg { .. }
                            | NodeKind::FuncTask { .. }
                    )
                })
            }
            _ => None,
        }
    }

    /// Normalize a Slang instantiated subroutine declaration identity to the
    /// definition identity used by the current function context. The match is
    /// structural: same subroutine signature plus formal ordinal. This avoids
    /// redirecting an unrelated same-named declaration.
    pub(in super::super) fn canonical_func_target(&self, node: NodeId) -> Option<NodeId> {
        let definition = self.func.as_ref()?.def_node?;
        if node == definition {
            return Some(node);
        }
        match self.kind(node) {
            NodeKind::FuncTask { .. } => self
                .same_subroutine_signature(node, definition)
                .then_some(definition),
            NodeKind::FuncArg { .. } => {
                let owner = self.enclosing_func_task(node)?;
                if !self.same_subroutine_signature(owner, definition) {
                    return None;
                }
                let ordinal = self
                    .node(owner)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .position(|child| *child == node)?;
                self.node(definition)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .nth(ordinal)
                    .copied()
            }
            NodeKind::Var { .. } => {
                let owner = self.enclosing_func_task(node)?;
                let has_numeric_or_native_result = matches!(
                    self.kind(owner),
                    NodeKind::FuncTask {
                        is_task: false,
                        ret: Some(_),
                        ..
                    }
                );
                // Return variables are direct typed children of their
                // function. Block locals can also appear in the subroutine's
                // structural child list, but retain their statement-block
                // parent and must not be canonicalized to the function.
                let is_result = has_numeric_or_native_result
                    && self.node(node).parent == Some(owner)
                    && self.node(node).name == self.node(owner).name;
                (is_result && self.same_subroutine_signature(owner, definition))
                    .then_some(definition)
            }
            NodeKind::Array { .. } => {
                let owner = self.enclosing_func_task(node)?;
                let is_array_result = matches!(
                    self.kind(owner),
                    NodeKind::FuncTask {
                        is_task: false,
                        ret: Some(ret),
                        ..
                    } if ret.kind == "array"
                ) && self.node(node).parent == Some(owner)
                    && self.node(node).name == self.node(owner).name;
                (is_array_result && self.same_subroutine_signature(owner, definition))
                    .then_some(definition)
            }
            _ => None,
        }
    }

    pub(in super::super) fn enclosing_func_task(&self, node: NodeId) -> Option<NodeId> {
        let mut parent = self.node(node).parent;
        while let Some(candidate) = parent {
            if matches!(self.kind(candidate), NodeKind::FuncTask { .. }) {
                return Some(candidate);
            }
            parent = self.node(candidate).parent;
        }
        None
    }

    fn same_subroutine_signature(&self, left: NodeId, right: NodeId) -> bool {
        let same_kind = match (self.kind(left), self.kind(right)) {
            (
                NodeKind::FuncTask {
                    is_task: left_task,
                    ret: left_return,
                    ..
                },
                NodeKind::FuncTask {
                    is_task: right_task,
                    ret: right_return,
                    ..
                },
            ) => {
                left_task == right_task
                    && left_return
                        .as_ref()
                        .map(|ty| (&ty.kind, ty.width, ty.signed))
                        == right_return
                            .as_ref()
                            .map(|ty| (&ty.kind, ty.width, ty.signed))
            }
            _ => false,
        };
        if !same_kind || self.node(left).name != self.node(right).name {
            return false;
        }
        let formals = |subroutine: NodeId| {
            self.node(subroutine)
                .children
                .iter()
                .filter_map(|child| match self.kind(*child) {
                    NodeKind::FuncArg {
                        direction,
                        ty,
                        const_ref,
                        ref_static,
                        ..
                    } => Some((
                        *direction,
                        ty.kind.clone(),
                        ty.width,
                        ty.signed,
                        *const_ref,
                        *ref_static,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        formals(left) == formals(right)
    }
}
