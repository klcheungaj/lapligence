//! Calls.

use super::super::collection::{
    member_select_statement, CallWriteback, RecordRefDispatch, RecordRefPin, RecordRefSelect,
    TargetSelectors,
};
use super::*;

impl EmitCtx<'_, '_> {
    /// A waited record `ref` actual with neither a binding nor a marker.
    fn record_wait_error(&self, call: NodeId, name: &str) -> String {
        let node = self.cg.node(call);
        format!(
            "task `{name}` waits on a native record `ref` formal, so each of its record actuals must be a module, static or procedural-block record or an element of an array or container (SIM-009) at {}:{}:{} in `{}`",
            node.file.as_deref().unwrap_or("<unknown>"),
            node.line,
            node.col,
            self.cg.source_path(&self.path)
        )
    }

    /// Lower a `task_call` statement (or a function call used as a statement).
    /// Tasks and functions become IR calls with caller-side temps for output
    /// formals; timed callees are stackless coroutines and a call that may
    /// disable is followed by a cancellation check that skips copy-out.
    pub(super) fn lower_task_call(
        &mut self,
        h: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<IrStmt, String> {
        if let Some(statement) = self.cg.lower_object_random_statement(&self.path, h)? {
            return Ok(statement);
        }
        // Covergroup `sample`, `start`, `stop` and `set_inst_name` (SIM-044).
        if let Some(statement) = self.cg.lower_coverage_statement(&self.path, h)? {
            return Ok(statement);
        }
        if let Some(f) = &self.func {
            if is_task && !f.is_task
                // A constructor invocation is a task-shaped call in Slang's
                // snapshot, but it is legal while lowering a class
                // constructor function.  Use the active receiver rather
                // than the call-site parent: `super.new`'s callee is owned by
                // the base class and therefore is not always recognized by
                // `is_class_method_call`.
                && !(name == "new" && f.class_receiver.is_some())
            {
                return Err(format!(
                    "task call `{name}` inside function `{}` is not supported",
                    f.name
                ));
            }
        }
        let virtual_call_info = self.cg.virtual_interface_method_info(h)?;
        let (ft, callee_inst) = if let Some((_, _, ft, callee_inst, _)) = virtual_call_info {
            (ft, callee_inst)
        } else {
            self.cg
                .resolve_callee_env(self.inst, name, is_task, callee)?
        };
        let automatic = matches!(
            self.cg.kind(ft),
            NodeKind::FuncTask {
                automatic: true,
                ..
            }
        );
        if is_task
            && self.cg.func_body(ft).is_some_and(|body| {
                self.cg
                    .node_has_stack_backed_subroutine_nba(body, ft, automatic)
            })
        {
            return Err(format!(
                "nonblocking assignment in task `{name}` targets stack-backed input/formal/local storage which cannot outlive the call"
            ));
        }
        // All function/task definitions carry names; the model index exists
        // only for emitted (delay-free) callees.
        self.cg
            .func_names
            .get(&ft)
            .ok_or_else(|| format!("task `{name}` has no C name"))?;
        let (_, _, formals) = self.cg.func_info(ft, callee_inst)?;
        let args = self.cg.call_argument_nodes(h);
        let bound = self.cg.bind_call_args(self.inst, &formals, &args)?;
        let receiver = if virtual_call_info.is_some() {
            None
        } else {
            self.cg.class_method_receiver(&self.path, h)?
        };
        let virtual_call = if let Some((descriptor, method, _, _, receiver)) = virtual_call_info {
            Some(crate::sim::ir::IrVirtualCall {
                interface: descriptor,
                method,
                receiver: self.cg.lower_chandle(&self.path, receiver)?,
                site: self
                    .cg
                    .source_site("virtual interface method call", &self.path, h),
            })
        } else {
            None
        };
        let mut call_receiver = LoweredCallReceiver {
            class: receiver,
            virtual_interface: virtual_call,
        };
        // Every call reaches the typed body (see `CallShape`): a waited
        // `ref` formal whose actuals are whole module signals selects a
        // specialization that binds them; any other actual is followed
        // through its descriptor.
        let shape = self.cg.call_shape(ft, callee_inst);
        // Module, static and block records bound to native record `ref`
        // formals select a specialization (`record_refs`).
        let records = self.cg.record_ref_bindings(&formals, &bound);
        let dispatch = self.cg.record_ref_dispatch(h, virtual_call_info.is_some());
        // A column formal bound to a record that holds native members (an
        // element, or a record property) reads it through its address, which
        // publishes no change: a waiter on the formal could never resume.
        let unwatched_cell = records.iter().find(|record| {
            record.cell
                && self.cg.fixed_value_width(record.formal).is_none()
                && formals
                    .iter()
                    .position(|(formal, _)| *formal == record.formal)
                    .is_some_and(|index| match dispatch {
                        RecordRefDispatch::Virtual => {
                            self.cg.any_override_waits_on_record_ref(ft, callee_inst)
                        }
                        _ => self.cg.waits_on_ref_formal(ft, callee_inst, index),
                    })
        });
        if let Some(record) = unwatched_cell {
            let actual = formals
                .iter()
                .position(|(formal, _)| *formal == record.formal)
                .map_or(h, |index| bound[index].expr);
            let node = self.cg.node(actual);
            return Err(format!(
                "task `{name}` waits on record `ref` formal `{}` of the column form (16 or more cells), whose actual holds string, real, handle or container members and publishes no change that form follows: the wait could never resume (SIM-009) at {}:{}:{} in `{}`",
                self.cg.node(record.formal).name,
                node.file.as_deref().unwrap_or("<unknown>"),
                node.line,
                node.col,
                self.cg.source_path(&self.path)
            ));
        }
        // A member of a subroutine record with container members takes the
        // record's companions, whose changes publish nothing a waiter on the
        // formal follows (SIM-009).
        let waited = match dispatch {
            RecordRefDispatch::Virtual
                if self.cg.any_override_waits_on_record_ref(ft, callee_inst) =>
            {
                self.cg.native_record_formals(&formals)
            }
            RecordRefDispatch::Virtual => Vec::new(),
            _ => shape.static_refs.clone(),
        };
        for index in waited {
            let (Some((formal, _)), Some(actual)) = (formals.get(index), bound.get(index)) else {
                continue;
            };
            if self
                .cg
                .subroutine_record_container_member(&self.path, *formal, actual.expr)?
            {
                let node = self.cg.node(actual.expr);
                return Err(format!(
                    "task `{name}` waits on record `ref` formal `{}`, whose actual is a member of a subroutine record with queue or dynamic-array members; changes to the record publish no change the wait could follow (SIM-009) at {}:{}:{} in `{}`",
                    self.cg.node(*formal).name,
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.cg.source_path(&self.path)
                ));
            }
        }
        if let Some(select) = self.cg.record_ref_select(&self.path, &formals, &bound)? {
            let waits = match dispatch {
                RecordRefDispatch::Virtual => {
                    self.cg.any_override_waits_on_record_ref(ft, callee_inst)
                }
                _ => self.cg.waits_on_record_ref(ft, callee_inst),
            };
            return self.lower_per_record_element(h, select, waits, &mut |ctx| {
                ctx.lower_task_call(h, name, is_task, callee)
            });
        }
        let mut specialized = None;
        if !shape.static_refs.is_empty()
            && (records.is_empty() || dispatch == RecordRefDispatch::Direct)
        {
            if let Some(statics) =
                self.static_ref_actuals(&formals, &bound, &shape.static_refs, &records)?
            {
                let watched =
                    self.cg
                        .unbound_record_formals(&formals, &shape.static_refs, &records);
                specialized = Some(self.cg.task_specialization_watching(
                    ft,
                    callee_inst,
                    statics,
                    records.clone(),
                    watched,
                )?);
            }
        }
        if let Some(call) = call_receiver
            .virtual_interface
            .as_mut()
            .filter(|_| !records.is_empty())
        {
            let (method, function) = self.cg.record_ref_interface_method(
                call.interface,
                call.method,
                ft,
                &records,
                Vec::new(),
            )?;
            call.method = method;
            specialized = Some(function);
        }
        if specialized.is_none() && !records.is_empty() {
            let template = self
                .cg
                .func_meta
                .get(&ft)
                .map(|meta| meta.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            specialized = Some(self.cg.record_ref_callee(
                ft,
                callee_inst,
                &formals,
                &bound,
                template,
                dispatch,
            )?);
        }
        let waits = match dispatch {
            RecordRefDispatch::Virtual => self.cg.any_override_waits_on_record_ref(ft, callee_inst),
            _ => self.cg.waits_on_record_ref(ft, callee_inst),
        };
        // A waited record no specialization binds is followed through its
        // reference's marker: an element, or a formal that has one.
        if specialized.is_none() && waits {
            // Every instance of an interface runs the same method body.
            let waited = match dispatch {
                RecordRefDispatch::Direct | RecordRefDispatch::Interface => {
                    shape.static_refs.clone()
                }
                _ => self.cg.native_record_formals(&formals),
            };
            let watched = self.cg.watched_record_formals(&formals, &waited);
            if watched.is_empty() {
                return Err(self.record_wait_error(h, name));
            }
            // A virtual-interface method dispatches to one watching
            // specialization per interface instance.
            if let Some(call) = call_receiver.virtual_interface.as_mut() {
                let (method, function) = self.cg.record_ref_interface_method(
                    call.interface,
                    call.method,
                    ft,
                    &[],
                    watched.clone(),
                )?;
                call.method = method;
                specialized = Some(function);
            }
            specialized = Some(if let Some(function) = specialized {
                function
            } else if dispatch == RecordRefDispatch::Virtual {
                self.cg.record_ref_family(ft, Vec::new(), watched)?
            } else {
                self.cg.task_specialization_watching(
                    ft,
                    callee_inst,
                    Vec::new(),
                    Vec::new(),
                    watched,
                )?
            });
        }
        for &index in &shape.native_refs {
            let actual = bound[index].expr;
            if !self.cg.native_ref_actual_observable(actual) {
                let node = self.cg.node(actual);
                return Err(format!(
                    "`ref` actual of string or handle formal `{}` of task `{name}`, which waits on it, is neither a variable nor a class property; its changes publish no marker the task can wait on (SIM-009) at {}:{}:{} in `{}`",
                    self.cg.node(formals[index].0).name,
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.cg.source_path(&self.path)
                ));
            }
        }
        let can_be_disabled = is_task
            && (self.cg.task_has_disable(ft, callee_inst) || self.cg.task_is_disable_target(ft));
        let fidx = match specialized {
            Some(index) => index,
            None => self
                .cg
                .func_meta
                .get(&ft)
                .map(|m| m.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?,
        };
        let fidx = self.cg.dpi_open_callee(fidx, &formals, &bound)?;
        self.cg.record_ref_call_site = Some(h);
        let call = self.lower_call_stmts(fidx, callee_inst, h, &formals, &bound, call_receiver);
        self.cg.record_ref_call_site = None;
        let call = call?;
        // A class task's call keeps the activation only when a `disable`
        // names the task, matching its body (`emit_func_task_with`).
        if can_be_disabled
            && (!self.cg.class_nodes.contains_key(&callee_inst)
                || self.cg.task_is_disable_target(ft))
        {
            // Keep cancellation visible after the callee retires its own
            // activation, until caller-side output copy-out has finished.
            Ok(IrStmt::ActivationScope {
                target: self.cg.activation_target(ft)?,
                exit: self.new_label("call_exit"),
                body: vec![call],
            })
        } else {
            Ok(call)
        }
    }

    /// The first run-time selected record element bound to a native record
    /// `ref` formal by a function called inside an expression of statement
    /// `h`. Only statements that evaluate their expressions once, at their
    /// start, are lowered once per element (`lower_per_record_element`), so
    /// the index is taken when the statement starts; a loop header, a
    /// condition and a timed assignment keep the located rejection.
    pub(super) fn statement_record_ref_select(
        &self,
        h: NodeId,
    ) -> Result<Option<RecordRefSelect>, String> {
        if !matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { delay: None, .. })
                | NodeKind::Stmt(StmtKind::Return { .. })
                | NodeKind::SysCall { .. }
        ) {
            return Ok(None);
        }
        let mut pending = self.cg.node(h).children.clone();
        pending.reverse();
        while let Some(node) = pending.pop() {
            let call = match self.cg.kind(node) {
                NodeKind::FuncCall {
                    name,
                    is_task: false,
                    callee,
                    ..
                }
                | NodeKind::MethodCall { name, callee, .. } => Some((name, *callee)),
                _ => None,
            };
            if let Some((name, callee)) = call {
                let args = self.cg.call_argument_nodes(node);
                // Only a select can be a run-time record element.
                let selects = args.iter().any(|arg| {
                    matches!(
                        self.cg.kind(self.cg.p30_unwrap_cast(*arg)),
                        NodeKind::Expr(
                            ExprKind::ArraySelect { .. }
                                | ExprKind::BitSelect { .. }
                                | ExprKind::HierPath { .. }
                        )
                    )
                });
                let formals = selects
                    .then(|| {
                        self.cg
                            .resolve_callee_env(self.inst, name, false, callee)
                            .ok()
                    })
                    .flatten()
                    .and_then(|(function, _)| self.cg.func_meta.get(&function))
                    .map(|meta| meta.formals.clone());
                if let Some(formals) = formals {
                    if let Ok(bound) = self.cg.bind_call_args(self.inst, &formals, &args) {
                        if let Some(select) =
                            self.cg.record_ref_select(&self.path, &formals, &bound)?
                        {
                            if let Some(before) = self.effect_before(h, select.source, select.index)
                            {
                                let node = self.cg.node(before);
                                return Err(format!(
                                    "run-time selected record element as a `ref` actual of function `{name}` after a call or an assignment operator at {}:{}:{} in the same statement in `{}` is not supported: the element is chosen when the statement starts; move the earlier operand to a statement of its own (SIM-008)",
                                    node.file.as_deref().unwrap_or("<unknown>"),
                                    node.line,
                                    node.col,
                                    self.cg.source_path(&self.path)
                                ));
                            }
                            return Ok(Some(select));
                        }
                    }
                }
            }
            let children = &self.cg.node(node).children;
            pending.extend(children.iter().rev().copied());
        }
        Ok(None)
    }

    /// A call or an assignment operator of statement `h` that starts before
    /// `actual` in the source, does not enclose it and may write a variable
    /// its `index` reads: left-to-right evaluation runs it before the index,
    /// which the statement takes at its start.
    fn effect_before(&self, h: NodeId, actual: NodeId, index: NodeId) -> Option<NodeId> {
        let target = self.cg.node(actual);
        let at = (target.line, target.col);
        let encloses = |node: NodeId| self.subtree(node).contains(&actual);
        // An index that has effects of its own reads what they write.
        let read = self.cg.side_effect_free(index).then(|| {
            self.subtree(index)
                .into_iter()
                .filter_map(|node| match self.cg.kind(node) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => Some(*target),
                    NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                        refs.first().copied().flatten()
                    }
                    _ => None,
                })
                .collect::<HashSet<_>>()
        });
        let mut pending = self.cg.node(h).children.clone();
        while let Some(node) = pending.pop() {
            let start = self.cg.node(node);
            if Self::is_effect(self.cg.kind(node))
                && (start.line, start.col) < at
                && !encloses(node)
                && read
                    .as_ref()
                    .is_none_or(|read| self.may_write(node, read, &mut HashSet::new()))
            {
                return Some(node);
            }
            pending.extend(self.cg.node(node).children.iter().copied());
        }
        None
    }

    fn is_effect(kind: &NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::FuncCall { .. }
                | NodeKind::MethodCall { .. }
                | NodeKind::SysCall { .. }
                | NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Assignment
                        | Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement,
                    ..
                })
        )
    }

    /// Every node of the subtree at `node`.
    fn subtree(&self, node: NodeId) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        let mut pending = vec![node];
        while let Some(next) = pending.pop() {
            nodes.push(next);
            pending.extend(self.cg.node(next).children.iter().copied());
        }
        nodes
    }

    /// The variable a selection, member path or reference `node` names.
    fn storage_root(&self, node: NodeId) -> Option<NodeId> {
        match self.cg.kind(self.cg.p30_unwrap_cast(node)) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                let first = refs.first().copied().flatten()?;
                match self.cg.kind(first) {
                    NodeKind::Expr(_) => self.storage_root(first),
                    _ => Some(first),
                }
            }
            NodeKind::Expr(
                ExprKind::ArraySelect { base, .. } | ExprKind::BitSelect { base, .. },
            ) => self.storage_root(*base),
            NodeKind::Expr(_) => {
                let first = self.cg.node(node).children.first().copied()?;
                self.storage_root(first)
            }
            _ => None,
        }
    }

    /// Whether effect `node` may write a variable of `read`: an operand that
    /// names one (an output or `ref` actual, an assignment target, a
    /// built-in method's receiver), or a subroutine whose body may, through
    /// the calls it makes. An unresolved callee, one with no body the
    /// lowering can see, or a virtual method may write anything.
    fn may_write(&self, node: NodeId, read: &HashSet<NodeId>, seen: &mut HashSet<NodeId>) -> bool {
        let names = |operand: NodeId| {
            self.storage_root(operand)
                .is_some_and(|root| read.contains(&root))
        };
        if self
            .cg
            .node(node)
            .children
            .iter()
            .any(|child| names(*child))
        {
            return true;
        }
        let callee = match self.cg.kind(node) {
            NodeKind::FuncCall { callee: None, .. } => return true,
            NodeKind::FuncCall {
                callee: Some(callee),
                ..
            } => *callee,
            // A built-in method writes its receiver, an operand above.
            NodeKind::MethodCall { callee: None, .. } => return false,
            NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => {
                if self.cg.class_method_virtual_dispatch(node) {
                    return true;
                }
                *callee
            }
            _ => return false,
        };
        if !seen.insert(callee) {
            return false;
        }
        let Some(body) = self.cg.func_body(callee) else {
            return true;
        };
        self.subtree(body)
            .into_iter()
            .any(|inner| match self.cg.kind(inner) {
                NodeKind::Stmt(StmtKind::Assign { .. }) => self
                    .cg
                    .node(inner)
                    .children
                    .first()
                    .is_some_and(|target| names(*target)),
                kind if Self::is_effect(kind) => self.may_write(inner, read, seen),
                _ => false,
            })
    }

    /// Lower a call, or a statement holding one in an expression (`lower`),
    /// once per element of a run-time selected record `ref` actual, under a
    /// decision tree over the index taken at the call, or when the statement
    /// starts. An index selecting no element reaches the callee with a
    /// private default value (SV 7.4.6), or with index checks on stops the
    /// run at `site`. A task waiting on the formal has no body without a
    /// binding, so that index stops the run there either way.
    pub(super) fn lower_per_record_element(
        &mut self,
        site: NodeId,
        select: RecordRefSelect,
        waits: bool,
        lower: &mut dyn FnMut(&mut Self) -> Result<IrStmt, String>,
    ) -> Result<IrStmt, String> {
        let mut statements = Vec::new();
        let (index, width) =
            self.cg
                .record_ref_select_index(&self.path, &select, &mut statements)?;
        let span = self.cg.record_ref_pin_span;
        self.cg.record_ref_pin_span = span * select.pins.len();
        let source = select.source;
        let mut pinned = |ctx: &mut Self, pin: RecordRefPin| {
            ctx.cg.record_ref_pins.insert(source, pin);
            let call = lower(ctx);
            ctx.cg.record_ref_pins.remove(&source);
            call
        };
        let mut branches = Vec::with_capacity(select.pins.len());
        let mut result = Ok(());
        for (declared, pin) in select.pins {
            match pinned(self, pin) {
                Ok(call) => branches.push((declared, Some(call))),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        // With index checks on, an index that selects no element stops the
        // run; off, a callee that does not wait binds the default. A waited
        // record formal has no default binding to follow (SIM-009), so that
        // call stops either way.
        let otherwise = result.and_then(|()| {
            let message = if waits {
                crate::sim::ir::UNSELECTED_REF_MESSAGE
            } else {
                crate::sim::ir::UNSELECTED_REF_BIND_MESSAGE
            };
            let stop = IrStmt::Severity {
                level: crate::sim::ir::IrSeverityLevel::Fatal,
                // `\036` is the runtime's hint separator
                // (`crate::sim::ir::HINT_SEPARATOR`).
                fmt: format!("\"{message}\\036{}\"", crate::sim::ir::INDEX_CHECK_HINT),
                args: Vec::new(),
                scope: self.path.clone(),
                location: self.cg.source_location(site),
                fatal_finish_number: Some(0),
                runtime_failure: true,
                skip_finals: false,
            };
            if waits {
                return Ok(stop);
            }
            let value = self.cg.native_temporary(select.formal)?;
            Ok(IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::RuntimeQuery(crate::sim::ir::IrRuntimeQuery::IndexChecks),
                    1,
                    false,
                    None,
                ),
                then_: vec![stop],
                els: Some(vec![
                    IrStmt::NativeValueDeclare(value),
                    pinned(self, RecordRefPin::Default(value))?,
                ]),
                check: IrUniquePriorityCheck::None,
            })
        });
        self.cg.record_ref_pin_span = span;
        statements.extend(member_select_statement(
            branches,
            &index,
            width,
            Some(otherwise?),
        ));
        Ok(IrStmt::Block(statements))
    }

    /// The whole-signal actuals of the `ref` formals `needed` at one call, or
    /// `None` when any of them cannot be bound statically (a caller local, a
    /// select, an element or a member), so the typed body follows the
    /// descriptors instead. A native record formal is bound by its entry in
    /// `records` instead.
    fn static_ref_actuals(
        &mut self,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        needed: &[usize],
        records: &[RecordRefBinding],
    ) -> Result<Option<Vec<StaticRef>>, String> {
        let mut statics = Vec::with_capacity(needed.len());
        for &index in needed {
            let (formal, _) = formals[index];
            let actual = &bound[index];
            if self.cg.is_native_declaration(formal) {
                if records.iter().any(|binding| binding.formal == formal) {
                    continue;
                }
                return Ok(None);
            }
            let const_ref = match self.cg.kind(formal) {
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    const_ref,
                    ..
                } => *const_ref,
                _ => return Ok(None),
            };
            let Ok(lhs @ IrLhs::Whole(_)) = self.cg.lower_lhs(&self.path, actual.expr) else {
                return Ok(None);
            };
            let Some((width, signed, two_state, actual_const)) = self.cg.ref_lhs_type(&lhs) else {
                return Ok(None);
            };
            if (width, signed, two_state) != (actual.width, actual.signed, actual.two_state)
                || (actual_const && !const_ref)
            {
                return Ok(None);
            }
            statics.push(StaticRef {
                formal,
                read: self.cg.lower_expr(&self.path, actual.expr)?,
                dependencies: self.cg.collect_read_signals(&self.path, actual.expr)?,
                lhs,
                const_ref,
            });
        }
        Ok(Some(statics))
    }

    /// Make the `ref` actual just pushed onto `args` stop the run, with index
    /// checks on, when its index selects no element (as some simulators do;
    /// the text binds the default, which a waiter never sees change). An
    /// actual that always selects is left as it is.
    fn stop_if_unselected(
        &self,
        args: &mut [IrCallArg],
        io: NodeId,
        callee_inst: NodeId,
        index: usize,
        call: NodeId,
    ) {
        let Some(last) = args.last_mut() else { return };
        let Some(task) = self.cg.enclosing_func_task(io) else {
            return;
        };
        let stop = crate::sim::ir::IrRefStop {
            scope: self.path.clone(),
            location: self.cg.source_location(call),
            waited: self.cg.waits_on_ref_formal(task, callee_inst, index),
        };
        match last {
            IrCallArg::RefAddr {
                lhs,
                read,
                stop_unselected,
                ..
            } if matches!(read.kind(), IrExprKind::Container(_))
                || !matches!(lhs.as_ref(), IrLhs::Whole(_) | IrLhs::WholeRef { .. }) =>
            {
                *stop_unselected = Some(stop);
            }
            IrCallArg::ElementRef(reference)
                if reference.kind != crate::sim::ir::IrElementRefKind::Record
                    && !matches!(reference.root, crate::sim::ir::IrElementRoot::Value(_)) =>
            {
                reference.stop_unselected = Some(stop);
            }
            _ => {}
        }
    }

    /// Lower a delay-free task/function statement call: caller-side temps for
    /// output/inout formals followed by copy-out, and inputs by value.
    fn lower_call_stmts(
        &mut self,
        fidx: usize,
        callee_inst: NodeId,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        call_receiver: LoweredCallReceiver,
    ) -> Result<IrStmt, String> {
        let mut temps: Vec<(String, usize, Option<IrExpr>)> = Vec::new();
        let mut copyouts: Vec<(IrLhs, String, u32, bool)> = Vec::new();
        // The C signature orders all outputs first, then inputs — build the
        // argument list in that order, not by formal declaration index.
        let mut out_args: Vec<IrCallArg> = Vec::new();
        // Inputs are gathered by two passes; the formal index keeps them in
        // the callee's declaration order (the C parameter order).
        let mut in_args: Vec<(usize, IrCallArg)> = Vec::new();
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        let mut before = Vec::new();
        let mut after = Vec::new();
        // Columns after the first of column-layout record formals follow the
        // declared formals of each direction (see `record_formal_columns`).
        let mut record_out_args = Vec::new();
        let mut record_in_args = Vec::new();
        // Open-array formals bound below to storage of the actual's type.
        let mut open_storage = vec![false; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            // A DPI-C open-array formal whose actual crosses in storage of
            // its own type (outside the payload, or a dynamic array or
            // queue): the call's specialization has a formal of that type.
            if let Some(argument) = self.cg.dpi_open_storage_argument(*io, bound[idx].expr)? {
                open_storage[idx] = true;
                if *is_out {
                    out_args.push(argument);
                } else {
                    in_args.push((idx, argument));
                }
                continue;
            }
            if self.cg.is_subroutine_container(*io) {
                let argument = self.cg.container_call_argument(
                    &self.path,
                    *io,
                    bound[idx].expr,
                    Some(&mut before),
                    Some(&mut after),
                )?;
                // `ref` formals pass by address with the outputs.
                if *is_out || self.cg.is_ref_formal(*io) {
                    out_args.push(argument);
                } else {
                    in_args.push((idx, argument));
                }
                continue;
            }
            if self.cg.record_declaration(*io) {
                let address = *is_out
                    || matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            ..
                        }
                    );
                let mut columns = self
                    .cg
                    .record_call_columns(
                        &self.path,
                        *io,
                        bound[idx].expr,
                        Some((&mut before, &mut after)),
                    )?
                    .into_iter();
                let first = columns
                    .next()
                    .ok_or("column-layout record formal has no columns")?;
                if address {
                    out_args.push(first);
                    record_out_args.extend(columns);
                } else {
                    in_args.push((idx, first));
                    record_in_args.extend(columns);
                }
                continue;
            }
            if self.cg.is_native_declaration(*io) {
                let argument = self.cg.native_call_argument(
                    &self.path,
                    *io,
                    bound[idx].expr,
                    &mut before,
                    &mut after,
                )?;
                if self.cg.model.funcs[fidx].formals[idx].record_marker
                    && !self.cg.watchable_record_argument(&argument)
                {
                    return Err(self.record_wait_error(h, &self.cg.node(h).name));
                }
                if *is_out || self.cg.is_ref_formal(*io) {
                    out_args.push(argument);
                } else {
                    in_args.push((idx, argument));
                }
                continue;
            }
            if self.cg.real_formal_array(*io).is_some() {
                let argument = self
                    .cg
                    .real_array_argument(&self.path, *io, bound[idx].expr)?;
                if *is_out
                    || matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            ..
                        }
                    )
                {
                    out_args.push(argument);
                } else {
                    in_args.push((idx, argument));
                }
                continue;
            }
            if self.cg.fixed_formal_array(*io).is_some() {
                let argument = IrCallArg::FixedValue(Box::new(
                    self.cg.lower_fixed_value(&self.path, bound[idx].expr)?,
                ));
                if *is_out
                    || matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            ..
                        }
                    )
                {
                    out_args.push(argument);
                } else {
                    in_args.push((idx, argument));
                }
                continue;
            }

            // Output, inout and ref event formals bind the caller's handle.
            if bound[idx].is_event && (*is_out || self.cg.is_ref_formal(*io)) {
                let target = self.cg.event_target_of(bound[idx].expr).ok_or_else(|| {
                    format!(
                        "event actual for formal `{}` in `{}` is not a named event handle",
                        self.cg.node(*io).name,
                        self.path
                    )
                })?;
                if let Some(access) = target.access.filter(|_| *is_out) {
                    // An output or inout event formal bound to a class event
                    // property: the property holds only the object identity,
                    // so the callee works on a handle copied in at the call
                    // (as the caller's handle address does for variables)
                    // and its final handle is copied back into the property
                    // of the object the actual names at return (SV 13.5).
                    let slot = self
                        .cg
                        .class_event_slot(&self.path, access, target.declaration)?;
                    let temporary = format!("_evout_{}_{idx}", h.0);
                    before.push(IrStmt::EventCapture {
                        name: temporary.clone(),
                        source: IrEventRef::Handle(Box::new(IrChandleExpr::LocalRead(
                            slot.clone(),
                        ))),
                    });
                    after.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                        slot,
                        IrChandleExpr::EventObject(Box::new(IrEventRef::Captured(
                            temporary.clone(),
                        ))),
                    ))));
                    out_args.push(IrCallArg::EventAddr(IrEventRef::Captured(temporary)));
                    continue;
                }
                out_args.push(self.cg.event_handle_argument(&self.path, *io, &target)?);
                continue;
            }
            if matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
            ) {
                let is_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if (is_ref || *is_out) && self.cg.is_process_formal(*io) {
                    let copy_in = matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    );
                    if is_ref {
                        if let Some(argument) =
                            self.cg.handle_element_ref(&self.path, bound[idx].expr)?
                        {
                            out_args.push(argument);
                            self.stop_if_unselected(&mut out_args, *io, callee_inst, idx, h);
                            continue;
                        }
                    }
                    if let Some((prelude, argument, store)) = self.cg.process_formal_binding(
                        &self.path,
                        *io,
                        bound[idx].expr,
                        is_ref,
                        copy_in,
                        &format!("_pout_{}_{idx}", h.0),
                    )? {
                        before.extend(prelude);
                        after.extend(store);
                        out_args.push(argument);
                        continue;
                    }
                }
                if !is_ref && *is_out {
                    let copy_in = matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    );
                    if let Some((prelude, address, store)) = self.cg.handle_element_copy_out(
                        &self.path,
                        bound[idx].expr,
                        copy_in,
                        &format!("_hout_{}_{idx}", h.0),
                    )? {
                        before.extend(prelude);
                        after.push(store);
                        out_args.push(IrCallArg::ChandleAddr(address));
                        continue;
                    }
                }
                if is_ref {
                    if let Some(argument) =
                        self.cg.handle_element_ref(&self.path, bound[idx].expr)?
                    {
                        out_args.push(argument);
                        self.stop_if_unselected(&mut out_args, *io, callee_inst, idx, h);
                        continue;
                    }
                }
                if is_ref || *is_out {
                    let (target, _) = self.cg.lower_chandle_lvalue(&self.path, bound[idx].expr)?;
                    let address = self.cg.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                } else {
                    in_args.push((
                        idx,
                        IrCallArg::ChandleVal(self.cg.lower_handle_argument(
                            &self.path,
                            *io,
                            bound[idx].expr,
                        )?),
                    ));
                }
                continue;
            }
            let is_ref = matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if is_ref {
                let (const_ref, ref_static) = match self.cg.kind(*io) {
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref,
                        ref_static,
                        ..
                    } => (*const_ref, *ref_static),
                    _ => unreachable!("ref formal"),
                };
                if bound[idx].string {
                    if let Some(argument) =
                        self.cg
                            .string_element_ref(&self.path, bound[idx].expr, const_ref)?
                    {
                        out_args.push(argument);
                        self.stop_if_unselected(&mut out_args, *io, callee_inst, idx, h);
                        continue;
                    }
                    // A class string property is passed by the address of
                    // its object's field (SV 13.5.2).
                    let property = if self.cg.class_storage_field(bound[idx].expr).is_some() {
                        self.cg
                            .class_field_string_lvalue(&self.path, bound[idx].expr)?
                            .map(|name| format!("&{name}"))
                    } else {
                        None
                    };
                    if !const_ref && property.is_none() {
                        self.cg
                            .ensure_string_actual_writable(&self.path, bound[idx].expr)?;
                    }
                    let observed = self.cg.enclosing_func_task(*io).is_some_and(|task| {
                        self.cg
                            .call_shape(task, callee_inst)
                            .native_refs
                            .contains(&idx)
                    });
                    let addr = match property {
                        Some(addr) => addr,
                        None => self
                            .cg
                            .lower_string_actual_address(&self.path, bound[idx].expr)?,
                    };
                    out_args.push(IrCallArg::StringRefAddr {
                        addr,
                        const_ref,
                        observed,
                    });
                } else {
                    out_args.push(self.cg.lower_ref_arg(
                        &self.path,
                        &bound[idx],
                        const_ref,
                        ref_static,
                    )?);
                    self.stop_if_unselected(&mut out_args, *io, callee_inst, idx, h);
                    if self
                        .cg
                        .enclosing_func_task(*io)
                        .is_some_and(|task| self.cg.waits_on_ref_formal(task, callee_inst, idx))
                    {
                        self.cg.observe_waited_ref_actual(
                            &self.path,
                            bound[idx].expr,
                            out_args.last_mut(),
                        )?;
                    }
                }
                if !bound[idx].string && !matches!(out_args.last(), Some(IrCallArg::ElementRef(_)))
                {
                    let read_ir = self.cg.lower_expr(&self.path, bound[idx].expr)?;
                    arg_irs[idx] = Some(read_ir);
                }
                continue;
            }
            if !*is_out {
                continue;
            }
            if bound[idx].string {
                let copy_in = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                );
                if let Some((prelude, address, store)) = self.cg.string_element_copy_out(
                    &self.path,
                    bound[idx].expr,
                    copy_in,
                    &format!("_stout_{}_{idx}", h.0),
                )? {
                    before.extend(prelude);
                    after.push(store);
                    out_args.push(IrCallArg::StringOutAddr(address));
                    continue;
                }
                self.cg
                    .ensure_string_actual_writable(&self.path, bound[idx].expr)?;
                let writeback = self
                    .cg
                    .lower_string_actual_address(&self.path, bound[idx].expr)?;
                let init = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                )
                .then(|| self.cg.lower_string(&self.path, bound[idx].expr))
                .transpose()?;
                let (storage_addr, storage_read) = self
                    .cg
                    .static_string_formals
                    .get(&(callee_inst, *io))
                    .map(|object| {
                        (
                            Some(format!("&{}", self.cg.model.objects[*object].c_name)),
                            Some(Box::new(IrStringExpr::Read(*object))),
                        )
                    })
                    .unwrap_or((None, None));
                out_args.push(IrCallArg::StringOutTemp {
                    name: format!("_st{}_{}", h.0, idx),
                    init: init.map(Box::new),
                    writeback,
                    storage_addr,
                    storage_read,
                });
                continue;
            }
            let read_actual = matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Inout,
                    ..
                }
            );
            let (mut writeback, actual_read, selector_inits) = self.cg.lower_call_writeback(
                &self.path,
                bound[idx].expr,
                &format!("{}_{}", h.0, idx),
                read_actual,
                TargetSelectors::AtCopyOut,
            )?;
            for (name, width, signed, two_state, init) in selector_inits {
                before.push(IrStmt::DeclLocal {
                    name,
                    width,
                    signed,
                    two_state,
                    init: Some(Box::new(init)),
                });
            }
            before.extend(writeback.take_prelude());
            if let Some(storage) = self.cg.static_formals.get(&(callee_inst, *io)).cloned() {
                let storage_lhs = IrLhs::Whole(storage.ir);
                if read_actual {
                    let value = actual_read.clone().ok_or_else(|| {
                        "inout subroutine actual has no caller-side input value".to_owned()
                    })?;
                    before.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.cg.model, &storage_lhs, value),
                        lhs: storage_lhs.clone(),
                        nba: false,
                    });
                }
                let read = IrExpr::new(
                    IrExprKind::SigRead(storage.ir),
                    storage.width,
                    storage.signed,
                    None,
                );
                after.push(writeback.store(&self.cg.model, read.clone())?);
                arg_irs[idx] = Some(read);
                out_args.push(IrCallArg::OutAddr(format!("&{}", storage.global)));
                continue;
            }
            let lh = match writeback {
                CallWriteback::Lhs(lhs) => lhs,
                other => {
                    // The call temporary keeps the formal's typed default; its
                    // copy-out lands in a caller local that the checked unpack
                    // or container element store consumes after the call.
                    let local = format!("_sco{}_{}", h.0, idx);
                    let (width, signed) = (bound[idx].width, bound[idx].signed);
                    if bound[idx].real && matches!(other, CallWriteback::Stream(_)) {
                        return Err(format!(
                            "real output formal cannot copy out to a streaming concatenation in `{}`",
                            self.path
                        ));
                    }
                    before.push(IrStmt::DeclLocal {
                        name: local.clone(),
                        width,
                        signed,
                        two_state: bound[idx].two_state,
                        init: default_real_local_initializer(width),
                    });
                    after.push(other.store(
                        &self.cg.model,
                        IrExpr::new(IrExprKind::LocalRead(local.clone()), width, signed, None),
                    )?);
                    IrLhs::WholeRef {
                        addr: format!("&{local}"),
                        width,
                        signed,
                        two_state: bound[idx].two_state,
                        shortreal: false,
                    }
                }
            };
            let tname = format!("_a{}_{}", h.0, idx);
            let init_ir = self
                .cg
                .lower_call_temp_init_from_expr(*io, &bound[idx], actual_read)?;
            temps.push((tname.clone(), idx, init_ir));
            copyouts.push((lh, tname.clone(), bound[idx].width, bound[idx].signed));
            arg_irs[idx] = Some(IrExpr::new(
                IrExprKind::LocalRead(tname.clone()),
                bound[idx].width,
                bound[idx].signed,
                None,
            ));
            out_args.push(IrCallArg::OutAddr(format!("&{tname}")));
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if open_storage[idx]
                || self.cg.fixed_formal_array(*io).is_some()
                || self.cg.is_native_declaration(*io)
                || self.cg.is_subroutine_container(*io)
                || self.cg.real_formal_array(*io).is_some()
            {
                continue;
            }
            let is_ref = matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !*is_out
                && !is_ref
                && matches!(self.cg.kind(*io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind))
            {
                continue;
            }
            if !*is_out && !is_ref {
                if bound[idx].is_event {
                    let event = if self.cg.is_null_event_expression(bound[idx].expr) {
                        IrEventRef::Null
                    } else {
                        let target = self.cg.event_target_of(bound[idx].expr).ok_or_else(|| {
                            format!(
                                "event actual for formal `{}` is not a named event (node kind: {:?})",
                                self.cg.node(*io).name,
                                self.cg.kind(bound[idx].expr)
                            )
                        })?;
                        self.cg.event_ref_of(&target, &self.path)?
                    };
                    in_args.push((idx, IrCallArg::EventVal(event)));
                } else if bound[idx].string {
                    in_args.push((
                        idx,
                        IrCallArg::StringVal(self.cg.lower_string(&self.path, bound[idx].expr)?),
                    ));
                } else {
                    let ir =
                        self.cg
                            .lower_bound_arg(&self.path, formals, bound, idx, &mut arg_irs)?;
                    arg_irs[idx] = Some(IrExpr::new(
                        IrExprKind::LocalRead(crate::sim::ir::call_argument_name(idx)),
                        ir.width,
                        ir.signed,
                        None,
                    ));
                    in_args.push((idx, IrCallArg::Val(ir)));
                }
            }
        }
        // Formals beyond the declared ones and the extra record columns are
        // a trailing result.
        let declared = formals.len() + record_out_args.len() + record_in_args.len();
        // Trailing column-layout record outputs precede any trailing result
        // formal in the callee signature (see `record_formal_columns`).
        out_args.extend(record_out_args);
        if let Some(result) = self.cg.model.funcs[fidx]
            .formals
            .last()
            .and_then(|formal| formal.native_value)
            .filter(|_| self.cg.model.funcs[fidx].formals.len() > declared)
        {
            // A native result discarded by a statement call still needs
            // caller-owned result storage.
            let temporary = self.cg.native_temporary_like(result)?;
            before.push(IrStmt::NativeValueDeclare(temporary));
            out_args.push(IrCallArg::NativeValue(temporary));
        }
        if let Some(result) = self.cg.model.funcs[fidx]
            .formals
            .last()
            .and_then(|formal| formal.container)
            .filter(|_| self.cg.model.funcs[fidx].formals.len() > declared)
        {
            // A container result discarded by a statement call still needs
            // caller-owned result storage.
            let temporary = self.cg.container_temporary_like(result);
            before.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                temporary,
            ))));
            out_args.push(IrCallArg::Container(temporary));
        }
        if let Some(result) = self.cg.model.funcs[fidx]
            .formals
            .last()
            .and_then(|formal| formal.real_array)
            .filter(|_| self.cg.model.funcs[fidx].formals.len() > declared)
        {
            // A real-array result discarded by a statement call still needs
            // caller-owned result cells.
            let temporary = self.cg.real_array_temporary_like(result);
            before.push(IrStmt::FixedArrayDeclare(temporary));
            out_args.push(IrCallArg::RealArray(temporary));
        }
        in_args.sort_by_key(|(idx, _)| *idx);
        out_args.extend(in_args.into_iter().map(|(_, argument)| argument));
        out_args.extend(record_in_args);
        let depth = parse_depth(&self.depth_arg);
        let call = IrStmt::Call(Box::new(IrCall {
            f: fidx,
            args: out_args,
            depth,
            receiver: call_receiver.class,
            virtual_dispatch: self.cg.class_method_virtual_dispatch(h),
            virtual_call: call_receiver.virtual_interface,
            temps,
            copyouts,
            dpi_site: self.cg.dpi_call_site(fidx, h),
            vpi_call: self.cg.vpi_call_origin(h),
        }));
        if before.is_empty() && after.is_empty() {
            Ok(call)
        } else {
            before.push(call);
            before.extend(after);
            Ok(IrStmt::Block(before))
        }
    }

    /// Lower a `return` statement.  In a non-void function the value becomes
    /// the `_ret` conversion + C return (rendered); in a void function/task it
    /// is a bare return.
    ///
    /// Rewrite a lowered `return` of a subroutine with `end` block events:
    /// store the value as the return statement would, then jump to `exit`,
    /// which precedes the end samples and falls through to the epilogue.
    pub(super) fn return_to_coverage_exit(
        &mut self,
        statement: IrStmt,
        exit: &str,
    ) -> Result<IrStmt, String> {
        Ok(match statement {
            IrStmt::Return { value: None } => IrStmt::Goto(exit.to_owned()),
            IrStmt::Return { value: Some(value) } => {
                let ret = self
                    .func
                    .as_ref()
                    .and_then(|function| function.ret.clone())
                    .ok_or("value return in a subroutine without a result")?;
                let value = *value;
                let rhs = if ret.width == 0 {
                    if value.is_real() {
                        value
                    } else {
                        IrExpr::new(
                            IrExprKind::CastToReal {
                                a: Box::new(value),
                                shortreal: ret.shortreal,
                            },
                            REAL_EXPR_WIDTH,
                            true,
                            None,
                        )
                    }
                } else {
                    ir_to_storage(value, ret.width, ret.signed, ret.two_state)?
                };
                IrStmt::Block(vec![
                    IrStmt::Assign {
                        lhs: IrLhs::WholeRef {
                            addr: format!("&{}", ret.c_name),
                            width: ret.width,
                            signed: ret.signed,
                            two_state: ret.two_state,
                            shortreal: ret.shortreal,
                        },
                        rhs,
                        nba: false,
                    },
                    IrStmt::Goto(exit.to_owned()),
                ])
            }
            IrStmt::Block(statements) => IrStmt::Block(
                statements
                    .into_iter()
                    .map(|statement| self.return_to_coverage_exit(statement, exit))
                    .collect::<Result<_, _>>()?,
            ),
            statement => statement,
        })
    }

    pub(super) fn lower_return(&mut self, value: Option<NodeId>) -> Result<IrStmt, String> {
        if let Some(result) = self
            .func
            .as_ref()
            .and_then(|function| function.ret_node)
            .filter(|node| self.cg.native_return(*node))
        {
            let mut statements = Vec::new();
            if let Some(value) = value {
                statements.push(
                    self.cg
                        .lower_native_value_assignment(
                            &self.path,
                            result,
                            value,
                            false,
                            Operation::Assignment,
                        )?
                        .ok_or("native return has no record assignment")?,
                );
            }
            statements.push(IrStmt::Return { value: None });
            return Ok(IrStmt::Block(statements));
        }
        if let Some(result) = self
            .func
            .as_ref()
            .and_then(|function| function.ret_node)
            .filter(|node| self.cg.container_return(*node))
        {
            let mut statements = Vec::new();
            if let Some(value) = value {
                statements.push(
                    self.cg
                        .lower_container_assignment(
                            &self.path,
                            result,
                            value,
                            true,
                            Operation::Assignment,
                        )?
                        .ok_or("container return has no container assignment")?,
                );
            }
            statements.push(IrStmt::Return { value: None });
            return Ok(IrStmt::Block(statements));
        }
        if let Some(result) = self
            .func
            .as_ref()
            .and_then(|function| function.ret_node)
            .filter(|node| self.cg.record_declaration(*node))
        {
            let mut statements = Vec::new();
            if let Some(value) = value {
                statements.push(
                    self.cg
                        .lower_column_record_assignment(
                            &self.path,
                            result,
                            value,
                            false,
                            Operation::Assignment,
                        )?
                        .ok_or("column-layout record return has no record assignment")?,
                );
            }
            statements.push(IrStmt::Return { value: None });
            return Ok(IrStmt::Block(statements));
        }
        if let Some(result) = self
            .func
            .as_ref()
            .and_then(|function| function.ret_node)
            .filter(|node| {
                self.cg.fixed_formal_array(*node).is_some()
                    || self.cg.real_formal_array(*node).is_some()
            })
        {
            let mut statements = Vec::new();
            if let Some(value) = value {
                statements.push(
                    self.cg
                        .lower_p30_fixed_array_assignment(
                            &self.path,
                            result,
                            value,
                            true,
                            Operation::Assignment,
                        )?
                        .ok_or("fixed return has no descriptor assignment")?,
                );
            }
            statements.push(IrStmt::Return { value: None });
            return Ok(IrStmt::Block(statements));
        }
        let returns_string = self.func.as_ref().is_some_and(|function| {
            function.ret_node.is_some_and(|node| {
                function
                    .string_write
                    .get(&node)
                    .is_some_and(|target| target == "_ret")
            })
        });
        if returns_string {
            return match value {
                Some(value) => Ok(IrStmt::Block(vec![
                    IrStmt::Object(Box::new(crate::sim::ir::IrObjectStmt::StringAssignLocal(
                        "_ret".to_string(),
                        self.cg.lower_string(&self.path, value)?,
                    ))),
                    IrStmt::Return { value: None },
                ])),
                None => Ok(IrStmt::Return { value: None }),
            };
        }
        let returns_chandle = self.func.as_ref().is_some_and(|function| {
            function.ret_node.is_some_and(|node| {
                matches!(function.chandle_write.get(&node),
                    Some(ChandleTarget::Local(name)) if name == "_ret")
            })
        });
        if returns_chandle {
            return match value {
                Some(value) => Ok(IrStmt::Block(vec![
                    IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                        "_ret".to_owned(),
                        self.cg.lower_chandle(&self.path, value)?,
                    ))),
                    IrStmt::Return { value: None },
                ])),
                None => Ok(IrStmt::Return { value: None }),
            };
        }
        match self.func.as_ref() {
            Some(f) if f.ret.is_some() => {
                let r = f.ret.as_ref().expect("ret width known").clone();
                match value {
                    Some(v) => {
                        let e = self.cg.lower_expr(&self.path, v)?;
                        let e = apply_assignment_expression_width(e, r.width);
                        Ok(IrStmt::Return {
                            value: Some(Box::new(e)),
                        })
                    }
                    None => Ok(IrStmt::Return { value: None }),
                }
            }
            Some(_) => Ok(IrStmt::Return { value: None }),
            None => Err(format!(
                "return statement outside a function/task in `{}`",
                self.path
            )),
        }
    }
}
