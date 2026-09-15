//! Dispatch.

use super::*;

impl EmitCtx<'_, '_> {
    /// Lower one statement (or construct) into its IR statements.  Mirrors
    /// the pre-IR emitter decision-for-decision: same errors, warnings,
    /// sensitivity sets and wait tracking.
    pub(in super::super) fn lower_stmt(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Begin) => {
                let mut body = Vec::new();
                let children = self.cg.node(h).children.clone();
                if self.func.is_none() {
                    for child in &children {
                        if matches!(self.cg.kind(*child), NodeKind::Var { .. }) {
                            if self.cg.is_mailbox_expr(&self.path, *child) {
                                if self.cg.db.variable_lifetime(*child) == VariableLifetime::Static
                                {
                                    self.cg.collect_mailbox_static_object(&self.path, *child)?;
                                    continue;
                                }
                                let name = self.cg.collect_mailbox_local(&self.path, *child)?;
                                body.push(IrStmt::Object(IrObjectStmt::ChandleDeclareLocal(
                                    name.clone(),
                                    None,
                                )));
                                if let Some(initializer) = self.cg.db.var_initializer(*child) {
                                    let value = self.cg.lower_mailbox_expr(
                                        &self.path,
                                        initializer,
                                        self.cg.mailbox_element_for_decl(*child),
                                    )?;
                                    body.push(IrStmt::Object(IrObjectStmt::MailboxAssignLocal(
                                        name, value,
                                    )));
                                }
                                continue;
                            }
                            if matches!(
                                self.cg.kind(*child),
                                NodeKind::Var { ty }
                                    if ty.kind == "class"
                                        && ty.type_name.as_deref() == Some("semaphore")
                            ) {
                                let target = self.cg.collect_semaphore_local(&self.path, *child)?;
                                let init = self
                                    .cg
                                    .db
                                    .var_initializer(*child)
                                    .map(|initializer| {
                                        self.cg.lower_chandle(&self.path, initializer)
                                    })
                                    .transpose()?;
                                match target {
                                    ChandleTarget::Local(name) => body.push(IrStmt::Object(
                                        IrObjectStmt::ChandleDeclareLocal(name, init),
                                    )),
                                    ChandleTarget::Object(_) => {}
                                }
                                continue;
                            }
                            if matches!(
                                self.cg.kind(*child),
                                NodeKind::Var { ty }
                                    if ty.kind == "class"
                                        && ty.type_name.as_deref() == Some("process")
                            ) {
                                match self.cg.collect_process_local(&self.path, *child)? {
                                    ProcessTarget::Local(name) => body.push(IrStmt::Object(
                                        IrObjectStmt::ProcessDeclareLocal(name, None),
                                    )),
                                    ProcessTarget::Object(_) => {}
                                }
                                continue;
                            }
                            if matches!(self.cg.kind(*child), NodeKind::Var { ty } if ty.kind == "string")
                                && self.cg.is_foreach_iterator(*child)
                            {
                                let name = self.cg.collect_loop_string_var(&self.path, *child)?;
                                body.push(IrStmt::DeclString { name, init: None });
                                continue;
                            }
                            let info = self.cg.collect_loop_var(&self.path, *child)?;
                            if info.static_signal.is_none() {
                                body.push(IrStmt::DeclLocal {
                                    name: info.c_name,
                                    width: info.width,
                                    signed: info.signed,
                                    two_state: info.two_state,
                                    init: default_real_local_initializer(info.width),
                                });
                            }
                        }
                    }
                }
                // A named block is a potential `disable` target: its exit
                // label is allocated before the body lowers so disables
                // inside (including inside inlined task expansions) can
                // reference it.
                let named = !self.cg.node(h).name.is_empty();
                let activation_target = named.then(|| self.cg.activation_target(h)).transpose()?;
                let mut activation_exit = None;
                if named {
                    let exit = self.new_label("xb");
                    activation_exit = Some(exit.clone());
                    self.ctrl.push(CtrlScope::Block);
                }
                for s in &children {
                    // Local variable declarations inside the block are hoisted
                    // by the function-local collection; skip them here.
                    if matches!(
                        self.cg.kind(*s),
                        NodeKind::Var { .. } | NodeKind::FuncArg { .. }
                    ) {
                        // Native mailbox locals are emitted as object
                        // statements rather than function-entry packed
                        // storage. Nested blocks still need that declaration
                        // at their lexical entry.
                        if matches!(self.cg.kind(*s), NodeKind::Var { .. })
                            && self.cg.is_mailbox_expr(&self.path, *s)
                        {
                            body.extend(self.lower_variable_decl(*s)?);
                        }
                        continue;
                    }
                    body.extend(self.lower_stmt(*s)?);
                }
                if named {
                    match self.ctrl.pop() {
                        Some(CtrlScope::Block) => {}
                        _ => unreachable!("block scope stack imbalance"),
                    }
                }
                let statement = match (activation_target, activation_exit) {
                    (Some(target), Some(exit)) => IrStmt::ActivationScope { target, exit, body },
                    (None, None) => IrStmt::Block(body),
                    _ => unreachable!("named activation scope metadata mismatch"),
                };
                Ok(vec![statement])
            }
            NodeKind::Stmt(assertion @ StmtKind::ImmediateAssertion { .. }) => {
                self.lower_immediate_assertion(h, assertion)
            }
            NodeKind::Stmt(StmtKind::ConcurrentAssertion { kind, .. }) => {
                if matches!(kind, ConcurrentAssertionKind::Expect) {
                    if self.in_final {
                        return Err(format!(
                            "procedural expect cannot suspend inside a final block in `{}`",
                            self.path
                        ));
                    }
                    if self.timing_forbidden() {
                        return Err(format!(
                            "procedural expect cannot suspend inside a function in `{}`",
                            self.path
                        ));
                    }
                }
                let path = self.path.clone();
                self.cg.emit_concurrent_assertion(self.inst, &path, h)?;
                if matches!(kind, ConcurrentAssertionKind::Expect) {
                    Ok(vec![IrStmt::Expect {
                        identity: h.index() as u64,
                    }])
                } else {
                    Ok(Vec::new())
                }
            }
            NodeKind::Stmt(StmtKind::IfElse { cond, check }) => {
                let c = self.cg.lower_boolean_expr(&self.path, *cond)?;
                let then_node = self
                    .cg
                    .node(h)
                    .children
                    .get(1)
                    .copied()
                    .ok_or_else(|| "if without then branch".to_string())?;
                let then_ = self.lower_stmt(then_node)?;
                let els = match self.cg.node(h).children.get(2) {
                    Some(e) => Some(self.lower_stmt(*e)?),
                    None => None,
                };
                Ok(vec![IrStmt::If {
                    cond: c,
                    then_,
                    els,
                    check: lower_unique_priority_check(*check, self.cg.origin(h)),
                }])
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking, delay, ..
            }) => match delay {
                None => Ok(vec![self.lower_assignment(h, false)?]),
                Some(IntraControl::Delay(delay)) => {
                    if self.in_final {
                        return Err(format!(
                            "intra-assignment delay inside a final block in `{}` is not \
                             allowed (no timing controls in final)",
                            self.path
                        ));
                    }
                    let scaled = self.cg.lower_procedural_delay(&self.path, h, *delay)?;
                    self.lower_delayed_assignment(h, *blocking, scaled)
                }
                Some(
                    timing @ (IntraControl::Event { .. }
                    | IntraControl::Repeat { .. }
                    | IntraControl::Unsupported { .. }),
                ) => self.lower_event_assignment(h, *blocking, timing),
                Some(IntraControl::Cycle { count, .. }) => {
                    self.lower_cycle_assignment(h, *blocking, *count)
                }
            },
            NodeKind::Stmt(StmtKind::DelayControl { delay }) => {
                if self.in_final {
                    return Err(format!(
                        "`#delay` inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.timing_forbidden() {
                    return Err(format!(
                        "delay inside a function body in `{}` is not supported \
                         (ordinary functions cannot suspend)",
                        self.path
                    ));
                }
                let scaled = self.cg.lower_procedural_delay(&self.path, h, *delay)?;
                self.saw_wait = true;
                let mut out = vec![IrStmt::Delay { ticks: scaled }];
                // The body may be a `Stmt(Empty)` placeholder for a bare
                // `#N;`; skip it.
                if let Some(s) = self.cg.node(h).children.first() {
                    if !matches!(self.cg.kind(*s), NodeKind::Stmt(StmtKind::Empty)) {
                        out.extend(self.lower_stmt(*s)?);
                    }
                }
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::CycleDelayControl { count }) => {
                self.lower_cycle_delay(h, *count)
            }
            NodeKind::Stmt(StmtKind::EventControl { .. }) => {
                if self.in_final {
                    return Err(format!(
                        "`@(...)` event control inside a final block in `{}` is not \
                         allowed (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.timing_forbidden() {
                    return Err(format!(
                        "event control inside a function body in `{}` is not \
                         supported (ordinary functions cannot suspend)",
                        self.path
                    ));
                }
                self.lower_event_control(h)
            }
            NodeKind::Stmt(StmtKind::EventTrigger {
                blocking,
                target,
                timing,
            }) => {
                // A trigger never suspends its issuing process, so immediate
                // triggers remain valid inside function bodies. Nonblocking
                // triggers are kept distinct through IR and commit in NBA.
                let target = target.ok_or_else(|| {
                    format!(
                        "cannot resolve named event reference `{}` in `{}`",
                        self.cg.node(h).name,
                        self.path
                    )
                })?;
                let target = self.cg.event_target_of(target).ok_or_else(|| {
                    format!(
                        "cannot resolve named event expression `{}` in `{}`",
                        self.cg.node(h).name,
                        self.path
                    )
                })?;
                let event = self.cg.event_ref_of(&target, &self.path)?;
                if *blocking {
                    if let Some(timing) = timing {
                        return Err(self.unsupported_event_trigger_timing(
                            timing,
                            "blocking event triggers cannot carry timing controls",
                        ));
                    }
                    return Ok(vec![IrStmt::EventTrigger { ev: event }]);
                }
                self.lower_nonblocking_event_trigger(h, event, timing.as_ref())
            }
            NodeKind::Stmt(StmtKind::Case { .. }) => self.lower_case(h),
            NodeKind::Stmt(StmtKind::For { .. }) => self.lower_for(h),
            NodeKind::Stmt(StmtKind::While { cond, body }) => {
                let c = self.cg.lower_boolean_expr(&self.path, *cond)?;
                let (body, brk) = self.lower_loop_body(*body)?;
                // `continue` lands on the back edge (the condition test):
                // its label sits at the END of the body (lower_loop_body).
                let mut out = vec![IrStmt::While { cond: c, body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::DoWhile { cond, body }) => self.lower_do_while(*cond, *body),
            NodeKind::Stmt(StmtKind::Repeat { cond, body }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued repeat counts are not supported in `{}`",
                        self.path
                    ));
                }
                if !matches!(c.kind, IrExprKind::Const(_)) {
                    self.cg.warnings.push(format!(
                        "repeat count in `{}` is not a constant; evaluated at runtime",
                        self.path
                    ));
                }
                let (body, brk) = self.lower_loop_body(*body)?;
                let mut out = vec![IrStmt::Repeat { count: c, body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::Forever { body }) => {
                let (body, brk) = self.lower_loop_body(*body)?;
                let mut out = vec![IrStmt::Forever { body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::Wait { cond }) => {
                if self.in_final {
                    return Err(format!(
                        "`wait (...)` inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.timing_forbidden() {
                    return Err(format!(
                        "wait inside a function body in `{}` is not supported \
                         (ordinary functions cannot suspend)",
                        self.path
                    ));
                }
                if let Some(event) = self.triggered_event_ref(*cond)? {
                    let body = self.wait_body(h)?;
                    self.saw_wait = true;
                    return Ok(vec![IrStmt::WaitEventTriggered { event, body }]);
                }
                let c = self.cg.lower_expr(&self.path, *cond)?;
                let sens = self.cg.collect_read_signals(&self.path, *cond)?;
                self.saw_wait = true;
                // The body is optional (`wait (cond);`); the semantic database
                // captures it as a child only when the statement is present.
                // Skip the `Empty` placeholder like the delay/event controls.
                let mut body = Vec::new();
                if let Some(b) = self.cg.node(h).children.get(1) {
                    if !matches!(self.cg.kind(*b), NodeKind::Stmt(StmtKind::Empty)) {
                        body = self.lower_stmt(*b)?;
                    }
                }
                Ok(vec![IrStmt::WaitCond {
                    cond: c,
                    sens,
                    body,
                }])
            }
            NodeKind::Stmt(StmtKind::WaitOrder {
                events,
                if_true,
                if_false,
            }) => {
                if self.in_final {
                    return Err(format!(
                        "wait_order inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.timing_forbidden() {
                    return Err(format!(
                        "wait_order inside a function body in `{}` is not supported \
                         (ordinary functions cannot suspend)",
                        self.path
                    ));
                }
                if events.is_empty() {
                    return Err(format!("wait_order without events in `{}`", self.path));
                }
                let events = events
                    .iter()
                    .map(|event| {
                        let target = self.cg.event_target_of(*event).ok_or_else(|| {
                            format!("wait_order has an unresolved event in `{}`", self.path)
                        })?;
                        self.cg.event_ref_of(&target, &self.path)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let success = if_true
                    .map(|body| self.lower_stmt(body))
                    .transpose()?
                    .unwrap_or_default();
                let failure = if_false
                    .map(|body| self.lower_stmt(body))
                    .transpose()?
                    .unwrap_or_default();
                self.saw_wait = true;
                Ok(vec![IrStmt::WaitOrder {
                    events,
                    success,
                    failure,
                }])
            }
            NodeKind::Stmt(StmtKind::Force { .. }) => Ok(vec![self.lower_force(h)?]),
            NodeKind::Stmt(StmtKind::Release { .. }) => Ok(vec![self.lower_release(h)?]),
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => self.lower_proc_cont_assign(h),
            NodeKind::Stmt(StmtKind::Deassign { lhs }) => self.lower_deassign(*lhs),
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                self.lower_variable_decl(*declaration)
            }
            NodeKind::Stmt(StmtKind::Empty) => Ok(vec![IrStmt::Nop]),
            NodeKind::Stmt(StmtKind::Return { value }) => Ok(vec![self.lower_return(*value)?]),
            NodeKind::Stmt(StmtKind::Fork {
                target,
                join_kind,
                branches,
            }) => self.lower_fork(*target, *join_kind, branches),
            NodeKind::Stmt(StmtKind::WaitFork) => {
                // `wait fork;` suspends until every live fork group of the
                // current process has completed.
                if self.in_final {
                    return Err(format!(
                        "`wait fork` inside a final block in `{}` is not allowed \
                         (no timing controls or waits in final)",
                        self.path
                    ));
                }
                self.saw_wait = true;
                Ok(vec![IrStmt::WaitFork])
            }
            NodeKind::Stmt(StmtKind::DisableFork) => {
                // `disable fork;` kills every descendant of the current
                // process; the runtime discards their pending NBAs.
                Ok(vec![IrStmt::DisableFork])
            }
            NodeKind::Stmt(StmtKind::Break) | NodeKind::Stmt(StmtKind::Continue) => self
                .lower_break_continue(matches!(self.cg.kind(h), NodeKind::Stmt(StmtKind::Break))),
            NodeKind::Stmt(StmtKind::Disable { target }) => self.lower_disable(*target),
            NodeKind::Stmt(StmtKind::Foreach { .. }) => self.lower_foreach(h),
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if matches!(
                    *op,
                    Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement
                ) =>
            {
                Ok(vec![self.lower_inc_dec(*op, operands)?])
            }
            NodeKind::Expr(ExprKind::NewClass {
                is_super_class: true,
                constructor: None,
                ..
            }) => {
                // An implicit base constructor has no call-expression edge.
                // It still constructs every base layer before this layer's defaults.
                let receiver = self
                    .func
                    .as_ref()
                    .and_then(|function| function.class_receiver.clone())
                    .ok_or_else(|| {
                        format!("super constructor has no receiver in `{}`", self.path)
                    })?;
                self.cg
                    .lower_implicit_class_construction(&self.path, self.inst, receiver)
            }
            NodeKind::Expr(ExprKind::NewClass {
                is_super_class: true,
                constructor: Some(constructor),
                ..
            }) => {
                let (name, is_task, callee) = match self.cg.kind(*constructor) {
                    NodeKind::FuncCall {
                        name,
                        is_task,
                        callee,
                        ..
                    } => (name.clone(), *is_task, *callee),
                    _ => return Err("super constructor edge is not a function call".to_owned()),
                };
                let mut statements =
                    vec![self.lower_task_call(*constructor, &name, is_task, callee)?];
                let function = self.func.as_ref().and_then(|function| function.def_node);
                if function.is_some_and(|function| {
                    matches!(
                        self.cg.kind(function),
                        NodeKind::FuncTask {
                            is_constructor: true,
                            ..
                        }
                    )
                }) {
                    statements.extend(self.cg.lower_class_initializers(
                        &self.path,
                        self.inst,
                        IrChandleExpr::LocalRead("_this".to_owned()),
                    )?);
                }
                Ok(statements)
            }
            NodeKind::SysCall { name } => self.lower_sys_call(h, name),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                if self.in_final && *is_task {
                    return Err(format!(
                        "task call `{name}` inside a final block in `{}` is not allowed \
                         (final permits function statements only)",
                        self.path
                    ));
                }
                Ok(vec![self.lower_task_call(h, name, *is_task, *callee)?])
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                callee,
                ..
            } => {
                if (matches!(name.as_str(), "suspend" | "await")
                    && self.cg.is_process_expr(&self.path, *receiver))
                    || (name == "get" && self.cg.is_semaphore_expr(&self.path, *receiver))
                {
                    if self.in_final {
                        return Err(format!(
                            "blocking method `{name}` inside a final block in `{}` is not allowed",
                            self.path
                        ));
                    }
                    if self.timing_forbidden() {
                        return Err(format!(
                            "process method `{name}` inside a function body in `{}` is not supported",
                            self.path
                        ));
                    }
                    self.saw_wait = true;
                }
                if matches!(name.as_str(), "put" | "get" | "peek")
                    && self.cg.is_mailbox_expr(&self.path, *receiver)
                {
                    if self.in_final {
                        return Err(format!(
                            "blocking mailbox method `{name}` inside a final block in `{}` is not allowed",
                            self.path
                        ));
                    }
                    if self.timing_forbidden() {
                        return Err(format!(
                            "blocking mailbox method `{name}` inside a function body in `{}` is not supported",
                            self.path
                        ));
                    }
                    self.saw_wait = true;
                }
                if let Some(statement) = self.cg.lower_container_method(&self.path, h)? {
                    Ok(vec![statement])
                } else if let Some((_, _, ft, _, _)) = self.cg.virtual_interface_method_info(h)? {
                    let is_task =
                        matches!(self.cg.kind(ft), NodeKind::FuncTask { is_task: true, .. });
                    if self.in_final && is_task {
                        return Err(format!(
                            "task call `{name}` inside a final block in `{}` is not allowed \
                             (final permits function statements only)",
                            self.path
                        ));
                    }
                    let _ = (receiver, callee);
                    Ok(vec![self.lower_task_call(h, name, is_task, None)?])
                } else if self.cg.is_mailbox_expr(&self.path, *receiver) {
                    Ok(vec![self.cg.lower_mailbox_method(&self.path, h)?])
                } else if self.cg.is_class_method_call(h) {
                    let is_task = matches!(
                        callee.map(|callee| self.cg.kind(callee)),
                        Some(NodeKind::FuncTask { is_task: true, .. })
                    );
                    if self.in_final && is_task {
                        return Err(format!(
                            "task call `{name}` inside a final block in `{}` is not allowed \
                             (final permits function statements only)",
                            self.path
                        ));
                    }
                    let _ = receiver;
                    Ok(vec![self.lower_task_call(h, name, is_task, *callee)?])
                } else {
                    Ok(vec![self.cg.lower_object_method(&self.path, h)?])
                }
            }
            NodeKind::Stmt(StmtKind::Unsupported { object_type }) => {
                let node = self.cg.node(h);
                let file = node.file.as_deref().unwrap_or("<unknown>");
                Err(format!(
                    "unsupported executable statement kind {object_type:?} at \
                     {file}:{}:{} in `{}`",
                    node.line, node.col, self.path
                ))
            }
            other => Err(format!(
                "unsupported statement in `{}` (node kind {other:?})",
                self.path
            )),
        }
    }
}
