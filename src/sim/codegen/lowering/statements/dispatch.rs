//! Dispatch.

use super::*;
use crate::sim::ir::{IrProcessControl, IrRuntimeQuery};

/// Statements that run before and after a statement.
type Around = (Vec<IrStmt>, Vec<IrStmt>);

/// What [`EmitCtx::enter_statement`] replaced or prepared for one statement.
struct StatementEntry {
    /// Computed handles of the statement's operands, set before it.
    receivers: Vec<IrStmt>,
    receiver_roots: Vec<NodeId>,
    /// The enclosing statement's preludes.
    saved: Option<Vec<IrStmt>>,
    saved_mailbox: Option<(NodeId, Vec<IrStmt>)>,
    outer: Option<Vec<IrStmt>>,
    hoisted: Result<Vec<IrStmt>, String>,
    /// Copies of staged element members, in and out.
    staged: Option<Result<Around, String>>,
    /// Bindings of the views of unpacked interface members the statement
    /// names (`stage_interface_views`).
    interface_views: Option<Result<Vec<IrStmt>, String>>,
}

impl EmitCtx<'_, '_> {
    /// Lower one statement (or construct) into its IR statements.  Mirrors
    /// the pre-IR emitter decision-for-decision: same errors, warnings,
    /// sensitivity sets and wait tracking.
    pub(in super::super) fn lower_stmt(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        self.cg.check_stack(h)?;
        let check = self.cg.static_effect_check(h);
        self.cg
            .reject_slot_container_members(&self.path, h)
            .map_err(|error| self.cg.locate_error(h, error))?;
        let mut statements = self
            .lower_checked_stmt(h)
            .map_err(|error| self.cg.locate_error(h, error))?;
        statements.splice(0..0, check);
        Ok(statements)
    }

    fn lower_checked_stmt(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        // A function called in an expression with a run-time selected record
        // element as a `ref` actual: the statement lowers once per element.
        if let Some(select) = self.statement_record_ref_select(h)? {
            // An unselected index stops at the actual, as a column's cell
            // reference does.
            let site = select.source;
            return self
                .lower_per_record_element(site, select, false, &mut |ctx| {
                    ctx.lower_checked_stmt(h).map(IrStmt::Block)
                })
                .map(|statement| vec![statement]);
        }
        let entry = self.enter_statement(h)?;
        let lowered = match (&entry.interface_views, &entry.staged) {
            (Some(Err(_)), _) | (_, Some(Err(_))) => Ok(Vec::new()),
            _ => self.lower_stmt_operations(h),
        };
        self.leave_statement(h, entry, lowered)
    }

    /// Set the operand state statement `h` lowers under. Nested statements
    /// lower between this and [`Self::leave_statement`] without a frame of
    /// either (`crate::stack`).
    fn enter_statement(&mut self, h: NodeId) -> Result<StatementEntry, String> {
        let outer_receivers = std::mem::take(&mut self.cg.receiver_prelude);
        let receiver_roots = self.cg.qualify_class_containers(&self.path, h);
        let receivers = std::mem::replace(&mut self.cg.receiver_prelude, outer_receivers);
        let receiver_roots = receiver_roots?;
        // Container operands that cannot be built inside an expression are
        // built just before a statement that evaluates its operands once;
        // nested statements never inherit that prelude. Whole-value pattern
        // bindings of this statement declare their lexical storage before
        // it, so the bound value outlives the test.
        let hoist = matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { .. })
                | NodeKind::SysCall { .. }
                | NodeKind::FuncCall { .. }
                | NodeKind::MethodCall { .. }
        );
        let saved = std::mem::replace(&mut self.cg.container_call_prelude, hoist.then(Vec::new));
        let mailbox_hoist =
            hoist || matches!(self.cg.kind(h), NodeKind::Stmt(StmtKind::IfElse { .. }));
        let saved_mailbox = std::mem::replace(
            &mut self.cg.mailbox_statement_prelude,
            mailbox_hoist.then(|| (h, Vec::new())),
        );
        let outer = self.cg.record_binding_declarations.replace(Vec::new());
        // Unpacked interface members named through handles are bound before
        // a statement that evaluates its operands once, and before a
        // `foreach` for its header. A timed nonblocking assignment binds its
        // target at issue (SV 10.4.2); a timed blocking one binds the views
        // of its target again when its control has passed (SV 10.4.1).
        // Element staging below may locate elements in them.
        let timed_blocking = matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign {
                blocking: true,
                delay: Some(_),
                ..
            })
        );
        // A deferred report reads its operands again when it prints: each
        // operand binds its own views there.
        let deferred = matches!(
            self.cg.kind(h),
            NodeKind::SysCall { name } if super::deferred_display_task(name)
        );
        let views = timed_blocking
            || !deferred
                && matches!(
                    self.cg.kind(h),
                    NodeKind::Stmt(StmtKind::Assign { delay: None, .. })
                        | NodeKind::Stmt(StmtKind::Assign {
                            blocking: false,
                            delay: Some(
                                IntraControl::Delay(_)
                                    | IntraControl::Event { .. }
                                    | IntraControl::Repeat { .. }
                            ),
                            ..
                        })
                        | NodeKind::SysCall { .. }
                        | NodeKind::FuncCall { .. }
                        | NodeKind::MethodCall { .. }
                        | NodeKind::Expr(ExprKind::Operation { .. })
                        | NodeKind::Var { .. }
                        | NodeKind::Stmt(StmtKind::VariableDecl { .. })
                        | NodeKind::Stmt(StmtKind::Foreach { .. })
                );
        let interface_views = if views {
            self.cg
                .stage_interface_views(&self.path, h, false)
                .transpose()
        } else {
            None
        };
        if let (true, Some(Ok(bindings))) = (timed_blocking, &interface_views) {
            let lhs = self.cg.node(h).children[0];
            self.target_rebinds = self.cg.target_interface_bindings(lhs, bindings);
            // A target through a computed handle in a statement that may
            // rebind handles has a view of its own, bound only there.
            if let Some(placed) = self.cg.take_placed_interface_bindings(lhs) {
                self.placed_target_bindings = placed;
            }
        }
        // Queue and dynamic-array members of container record elements are
        // staged around statements that evaluate their operands once.
        let staging = matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { delay: None, .. })
                | NodeKind::SysCall { .. }
                | NodeKind::FuncCall { .. }
                | NodeKind::MethodCall { .. }
                | NodeKind::Expr(ExprKind::Operation { .. })
        ) && self.cg.staged_element_members.is_empty()
            && self.cg.staged_nested_elements.is_empty();
        // The value of a write through a nested associative array runs
        // before the array is bound.
        let hoisted = if staging && self.cg.element_staging_possible {
            self.cg.hoist_assignment_source(&self.path, h)
        } else {
            Ok(Vec::new())
        };
        let staged = if staging {
            Some(self.cg.stage_element_members(&self.path, h))
        } else {
            None
        };
        Ok(StatementEntry {
            receivers,
            receiver_roots,
            saved,
            saved_mailbox,
            outer,
            hoisted,
            staged,
            interface_views,
        })
    }

    /// Restore the state [`Self::enter_statement`] replaced and compose the
    /// statements that run `h`.
    fn leave_statement(
        &mut self,
        h: NodeId,
        entry: StatementEntry,
        lowered: Result<Vec<IrStmt>, String>,
    ) -> Result<Vec<IrStmt>, String> {
        let StatementEntry {
            receivers,
            receiver_roots,
            saved,
            saved_mailbox,
            outer,
            hoisted,
            staged,
            interface_views,
        } = entry;
        self.target_rebinds.clear();
        self.placed_target_bindings.clear();
        let mut lowered = lowered;
        let views = match interface_views {
            Some(Ok(views)) => {
                let ended = self.cg.end_interface_views();
                if let (Ok(_), Err(error)) = (&lowered, ended) {
                    lowered = Err(error);
                }
                views
            }
            Some(Err(error)) => {
                if let Some(Ok(_)) = &staged {
                    self.cg.end_element_member_staging();
                }
                return Err(error);
            }
            None => Vec::new(),
        };
        let (lowered, (before, after)) = match staged {
            Some(Err(error)) => (Err(error), Default::default()),
            Some(Ok(staging)) => {
                self.cg.end_element_member_staging();
                (lowered, staging)
            }
            None => (lowered, Default::default()),
        };
        self.cg.end_hoisted_source();
        let hoisted = hoisted?;
        let declarations =
            std::mem::replace(&mut self.cg.record_binding_declarations, outer).unwrap_or_default();
        let prelude = std::mem::replace(&mut self.cg.container_call_prelude, saved);
        let mailbox_prelude =
            std::mem::replace(&mut self.cg.mailbox_statement_prelude, saved_mailbox);
        let mut statements = lowered?;
        // An operand reached by another path than the expression lowering
        // would read a handle local that was never set.
        for root in receiver_roots {
            self.cg.receiver_roots.remove(&root);
            if !self.cg.receiver_roots_lowered.remove(&root) {
                return Err(self.cg.locate_error(h, format!(
                    "a class property selected through a computed handle in `{}` is not supported in this operand; assign the handle to a variable first",
                    self.path
                )));
            }
        }
        if let Some(mut prelude) = prelude.filter(|prelude| !prelude.is_empty()) {
            prelude.append(&mut statements);
            statements = prelude;
        }
        if let Some((_, mut prelude)) = mailbox_prelude.filter(|(_, prelude)| !prelude.is_empty()) {
            prelude.append(&mut statements);
            statements = prelude;
        }
        // Views are bound before anything else of the statement runs; a
        // declared variable outlives its statement, so its views are bound
        // in the enclosing scope.
        if !views.is_empty() {
            if matches!(
                self.cg.kind(h),
                NodeKind::Var { .. } | NodeKind::Stmt(StmtKind::VariableDecl { .. })
            ) {
                statements.splice(0..0, views);
            } else {
                statements = views.into_iter().chain(statements).collect();
                statements = vec![IrStmt::Block(statements)];
            }
        }
        // Staged members are copied out before the operand prelude and back
        // after the statement, in one lexical scope.
        if !hoisted.is_empty() || !before.is_empty() || !after.is_empty() {
            statements = vec![IrStmt::Block(
                hoisted
                    .into_iter()
                    .chain(before)
                    .chain(statements)
                    .chain(after)
                    .collect(),
            )];
        }
        if !declarations.is_empty() {
            statements.splice(0..0, declarations);
        }
        // Computed handles of the statement's operands are read by every
        // copy made above.
        if !receivers.is_empty() {
            if matches!(
                self.cg.kind(h),
                NodeKind::Var { .. } | NodeKind::Stmt(StmtKind::VariableDecl { .. })
            ) {
                // The declared variable outlives this statement.
                statements.splice(0..0, receivers);
            } else {
                statements = vec![IrStmt::Block(
                    receivers.into_iter().chain(statements).collect(),
                )];
            }
        }
        // Statement coverage counts the statement as it starts (40.2.2).
        if let Some(hit) = self.cg.statement_hit(h) {
            statements.insert(0, hit);
        }
        locate_suspensions(&mut statements, &self.cg.origin(h));
        Ok(statements)
    }

    /// Every nesting level of the source keeps one frame of this function on
    /// the stack (`crate::stack`), so the arms that hold values live in
    /// functions of their own.
    fn lower_stmt_operations(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        if let Some(statements) = self.cg.lower_random_statement(&self.path, h)? {
            return Ok(statements);
        }
        match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Begin) => self.lower_begin(h),
            NodeKind::Stmt(assertion @ StmtKind::ImmediateAssertion { .. }) => {
                self.lower_immediate_assertion(h, assertion)
            }
            // `restrict property` is not verified in simulation (16.15.4).
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind: ConcurrentAssertionKind::Restrict,
                ..
            }) => Ok(Vec::new()),
            // Reaching a procedural checker instantiation queues its static
            // assertions (17.3.1).
            NodeKind::Stmt(StmtKind::ProceduralChecker { instances }) => {
                let instances = instances.clone();
                self.cg.lower_procedural_checkers(&instances)
            }
            NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. }) => {
                self.lower_concurrent_assertion(h)
            }
            NodeKind::Stmt(StmtKind::IfElse { .. }) => self.lower_if_else(h),
            NodeKind::Stmt(StmtKind::Assign {
                blocking, delay, ..
            }) => self.lower_assign(h, *blocking, delay.as_ref()),
            NodeKind::Stmt(StmtKind::DelayControl { delay }) => self.lower_delay_control(h, *delay),
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
            NodeKind::Stmt(StmtKind::EventTrigger { .. }) => self.lower_event_trigger(h),
            NodeKind::Stmt(StmtKind::Case { .. }) => self.lower_case(h),
            NodeKind::Stmt(StmtKind::PatternCase { .. }) => self.lower_pattern_case(h),
            NodeKind::Stmt(StmtKind::RandCase { .. }) => self.lower_randcase(h),
            NodeKind::Stmt(StmtKind::RandSequence { .. }) => self.lower_randsequence(h),
            NodeKind::Stmt(StmtKind::For { .. }) => self.lower_for(h),
            NodeKind::Stmt(StmtKind::While { cond, body }) => self.lower_while(*cond, *body),
            NodeKind::Stmt(StmtKind::DoWhile { cond, body }) => self.lower_do_while(*cond, *body),
            NodeKind::Stmt(StmtKind::Repeat { cond, body }) => self.lower_repeat(*cond, *body),
            NodeKind::Stmt(StmtKind::Forever { body }) => self.lower_forever(*body),
            NodeKind::Stmt(StmtKind::Wait { cond }) => self.lower_wait(h, *cond),
            NodeKind::Stmt(StmtKind::WaitOrder { .. }) => self.lower_wait_order(h),
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                self.lower_variable_decl(*declaration)
            }
            NodeKind::Stmt(StmtKind::Return { value }) => self.lower_return_statement(*value),
            NodeKind::Stmt(StmtKind::Fork {
                target,
                join_kind,
                branches,
            }) => self.lower_fork(*target, *join_kind, branches),
            NodeKind::Stmt(StmtKind::Break) | NodeKind::Stmt(StmtKind::Continue) => self
                .lower_break_continue(matches!(self.cg.kind(h), NodeKind::Stmt(StmtKind::Break))),
            NodeKind::Stmt(StmtKind::Disable { target }) => self.lower_disable(*target),
            NodeKind::Stmt(StmtKind::Foreach { .. }) => self.lower_foreach(h),
            NodeKind::Expr(ExprKind::NewClass {
                is_super_class: true,
                constructor: Some(constructor),
                ..
            }) => self.lower_super_constructor(*constructor),
            NodeKind::SysCall { name } => self.lower_sys_call(h, name),
            NodeKind::MethodCall {
                receiver: Some(_), ..
            } => self.lower_method_statement(h),
            _ => self.lower_simple_statement(h),
        }
    }

    /// [`Self::lower_stmt_operations`] for the statements that hold no others.
    fn lower_simple_statement(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Force { .. }) => Ok(vec![self.lower_force(h)?]),
            NodeKind::Stmt(StmtKind::Release { .. }) => Ok(vec![self.lower_release(h)?]),
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => {
                Err(self.reject_procedural_assign(h, "assign"))
            }
            NodeKind::Stmt(StmtKind::Deassign { .. }) => {
                Err(self.reject_procedural_assign(h, "deassign"))
            }
            NodeKind::Stmt(StmtKind::Empty) => Ok(vec![IrStmt::Nop]),
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

    fn lower_forever(&mut self, body: NodeId) -> Result<Vec<IrStmt>, String> {
        let (body, brk) = self.lower_loop_body(body)?;
        let mut out = vec![IrStmt::Forever { body }];
        out.extend(brk);
        Ok(out)
    }

    /// Declare the variable `child` of a process block at its lexical entry.
    fn declare_block_variable(
        &mut self,
        child: NodeId,
        body: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        if self.cg.is_mailbox_expr(&self.path, child) {
            if self.cg.db.variable_lifetime(child) == VariableLifetime::Static {
                self.cg.collect_mailbox_static_object(&self.path, child)?;
                return Ok(());
            }
            let name = self.cg.collect_mailbox_local(&self.path, child)?;
            body.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                name.clone(),
                None,
            ))));
            if let Some(initializer) = self.cg.db.var_initializer(child) {
                let value = self.cg.lower_mailbox_expr(
                    &self.path,
                    initializer,
                    self.cg.mailbox_element_for_decl(child),
                )?;
                body.push(IrStmt::Object(Box::new(IrObjectStmt::MailboxAssignLocal(
                    name, value,
                ))));
            }
            return Ok(());
        }
        if matches!(
            self.cg.kind(child),
            NodeKind::Var { ty }
                if ty.kind == "class"
                    && ty.type_name.as_deref() == Some("semaphore")
        ) {
            let target = self.cg.collect_semaphore_local(&self.path, child)?;
            let init = self
                .cg
                .db
                .var_initializer(child)
                .map(|initializer| self.cg.lower_chandle(&self.path, initializer))
                .transpose()?;
            match target {
                ChandleTarget::Local(name) => body.push(IrStmt::Object(Box::new(
                    IrObjectStmt::ChandleDeclareLocal(name, init),
                ))),
                ChandleTarget::Object(_) => {}
            }
            return Ok(());
        }
        if matches!(
            self.cg.kind(child),
            NodeKind::Var { ty }
                if ty.kind == "class"
                    && ty.type_name.as_deref() == Some("process")
        ) {
            match self.cg.collect_process_local(&self.path, child)? {
                ProcessTarget::Local(name) => body.push(IrStmt::Object(Box::new(
                    IrObjectStmt::ProcessDeclareLocal(name, None),
                ))),
                ProcessTarget::Object(_) => {}
            }
            return Ok(());
        }
        if matches!(self.cg.kind(child), NodeKind::Var { ty } if ty.kind == "string")
            && self.cg.proc_string_local_name(child).is_some()
            && !self.cg.is_foreach_iterator(child)
        {
            // A string pattern variable of a case item
            // is declared with its process (SIM-007).
            return Ok(());
        }
        if matches!(self.cg.kind(child), NodeKind::Var { ty } if ty.kind == "string")
            && self.cg.is_foreach_iterator(child)
        {
            let name = self.cg.collect_loop_string_var(&self.path, child)?;
            body.push(IrStmt::DeclString { name, init: None });
            return Ok(());
        }
        if self.cg.block_records.contains_key(&child) || self.cg.is_block_native(child) {
            // Record leaves, strings and handles are
            // collected with the scope; the declaration
            // statement resets automatic ones at entry.
            return Ok(());
        }
        let info = self.cg.collect_loop_var(&self.path, child)?;
        if info.static_signal.is_none() {
            body.push(IrStmt::DeclLocal {
                name: info.c_name,
                width: info.width,
                signed: info.signed,
                two_state: info.two_state,
                init: default_real_local_initializer(info.width),
            });
        }
        Ok(())
    }

    fn lower_begin(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let mut body = Vec::new();
        let children = self.cg.node(h).children.clone();
        if self.func.is_none() {
            for child in &children {
                if matches!(self.cg.kind(*child), NodeKind::Var { .. }) {
                    self.declare_block_variable(*child, &mut body)?;
                }
            }
        }
        // A named block is a potential `disable` target: its exit
        // label is allocated before the body lowers so disables
        // inside can reference it.
        let named = !self.cg.node(h).name.is_empty();
        let activation_target = named.then(|| self.cg.activation_target(h)).transpose()?;
        let mut activation_exit = None;
        if named {
            let exit = self.new_label("xb");
            activation_exit = Some(exit.clone());
            self.ctrl.push(CtrlScope::Block);
        }
        // Covergroup block events on this named block (19.3): the
        // `begin` samples precede its first statement.
        body.extend(self.cg.coverage_block_samples(h, true)?);
        for s in &children {
            // Local variable declarations inside the block are hoisted
            // by the function-local collection; skip them here.
            if matches!(
                self.cg.kind(*s),
                NodeKind::Var { .. }
                    | NodeKind::Array { .. }
                    | NodeKind::FuncArg { .. }
                    | NodeKind::NamedEvent
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
            if crate::sim::semantic::is_declaration_only_member(self.cg.db, *s) {
                continue;
            }
            // A named block's scope symbol (and those of named blocks
            // nested in it) only declares; the block's own statement
            // owns the activation that `disable` targets. Lowering it
            // would enter and leave an empty activation on every run.
            if self.cg.db.semantic_detail(*s) == Some("StatementBlock") {
                continue;
            }
            body.extend(self.lower_stmt(*s)?);
        }
        // The `end` samples follow its last statement; a disable or
        // a jump out of the block skips them (decision S44-D8).
        body.extend(self.cg.coverage_block_samples(h, false)?);
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

    fn lower_concurrent_assertion(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::ConcurrentAssertion {
            kind,
            property,
            if_true,
            if_false,
            ..
        }) = self.cg.kind(h)
        else {
            return Err(format!("malformed concurrent assertion in `{}`", self.path));
        };
        let (property, if_true, if_false) = (*property, *if_true, *if_false);
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
        let expect = matches!(kind, ConcurrentAssertionKind::Expect);
        // 16.15.6.1: automatic variables and const casts take their
        // values when the instance is queued (an `expect`, when it
        // starts its one attempt; its action blocks run here).
        let sites = self
            .cg
            .queued_value_sites(if expect { property } else { h });
        let mut captures = Vec::with_capacity(sites.sources.len());
        for source in &sites.sources {
            let capture = self.cg.lower_expr(&path, *source)?;
            self.cg.check_queued_value(*source, &capture)?;
            captures.push(capture);
        }
        if !expect {
            // 16.15.6: reaching the statement queues an instance.
            let queue = self.cg.emit_procedural_concurrent_assertion(
                self.inst,
                &path,
                h,
                &sites.sites,
                captures,
            )?;
            return Ok(vec![queue]);
        }
        let previous = self.cg.enter_queued_values(&sites.sites, &captures);
        let result = self.cg.emit_concurrent_assertion(self.inst, &path, h);
        self.cg.queued_values = previous;
        result?;
        self.lower_expect_actions(h.index() as u64, if_true, if_false, captures)
    }

    fn lower_if_else(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::IfElse {
            predicate,
            if_true,
            if_false,
            check,
        }) = self.cg.kind(h)
        else {
            return Err(format!(
                "malformed conditional statement in `{}`",
                self.path
            ));
        };
        let c = self.cg.lower_conditional_predicate(&self.path, predicate)?;
        let then_ = self.lower_stmt(*if_true)?;
        let els = if_false.map(|branch| self.lower_stmt(branch)).transpose()?;
        Ok(vec![IrStmt::If {
            cond: c,
            then_,
            els,
            check: lower_unique_priority_check(*check, self.cg.origin(h)),
        }])
    }

    fn lower_assign(
        &mut self,
        h: NodeId,
        blocking: bool,
        delay: Option<&IntraControl>,
    ) -> Result<Vec<IrStmt>, String> {
        let statements = self.lower_timed_assign(h, blocking, delay)?;
        // An update waiting for an event or cycle control stays pending; the
        // runtime tells a zero delay from one that leaves the time slot.
        let same_slot = matches!(delay, None | Some(IntraControl::Delay(_)));
        if blocking || !same_slot || !self.update_ends_with_issuer(h) {
            return Ok(statements);
        }
        let bind = |operation| IrStmt::Container(Box::new(operation));
        Ok(std::iter::once(bind(IrContainerStmt::NbaBindOpen))
            .chain(statements)
            .chain([bind(IrContainerStmt::NbaBindClose)])
            .collect())
    }

    fn lower_timed_assign(
        &mut self,
        h: NodeId,
        blocking: bool,
        delay: Option<&IntraControl>,
    ) -> Result<Vec<IrStmt>, String> {
        match delay {
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
                self.lower_delayed_assignment(h, blocking, scaled)
            }
            Some(
                timing @ (IntraControl::Event { .. }
                | IntraControl::Repeat { .. }
                | IntraControl::Unsupported { .. }),
            ) => self.lower_event_assignment(h, blocking, timing),
            Some(IntraControl::Cycle { count, .. }) => {
                self.lower_cycle_assignment(h, blocking, *count)
            }
        }
    }

    fn lower_delay_control(&mut self, h: NodeId, delay: NodeId) -> Result<Vec<IrStmt>, String> {
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
        let scaled = self.cg.lower_procedural_delay(&self.path, h, delay)?;
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

    fn lower_event_trigger(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::EventTrigger {
            blocking,
            target,
            timing,
        }) = self.cg.kind(h)
        else {
            return Err(format!("malformed event trigger in `{}`", self.path));
        };
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
        // A nonblocking trigger resolves its event at issue, but only
        // a blocking one creates a missing associative entry: `->>`
        // on a missing entry creates nothing and triggers nothing
        // (decision S07-D14, the text is silent).
        let event = if *blocking {
            self.cg.bound_event_ref_of(&target, &self.path)?
        } else {
            self.cg.event_ref_of(&target, &self.path)?
        };
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

    fn lower_while(&mut self, cond: NodeId, body: NodeId) -> Result<Vec<IrStmt>, String> {
        let c = self.cg.lower_boolean_expr(&self.path, cond)?;
        let (body, brk) = self.lower_loop_body(body)?;
        // `continue` lands on the back edge (the condition test):
        // its label sits at the END of the body (lower_loop_body).
        let mut out = vec![IrStmt::While { cond: c, body }];
        out.extend(brk);
        Ok(out)
    }

    fn lower_repeat(&mut self, cond: NodeId, body: NodeId) -> Result<Vec<IrStmt>, String> {
        let c = self.lower_repeat_count(cond)?;
        if !matches!(c.kind, IrExprKind::Const(_)) {
            let message = format!(
                "repeat count in `{}` is not a constant; evaluated at runtime",
                self.path
            );
            self.cg.warn_at(cond, message);
        }
        let (body, brk) = self.lower_loop_body(body)?;
        let mut out = vec![IrStmt::Repeat { count: c, body }];
        out.extend(brk);
        Ok(out)
    }

    fn lower_wait(&mut self, h: NodeId, cond: NodeId) -> Result<Vec<IrStmt>, String> {
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
        if let Some(event) = self.triggered_event_ref(cond)? {
            let body = self.wait_body(h)?;
            self.saw_wait = true;
            return Ok(vec![IrStmt::WaitEventTriggered { event, body }]);
        }
        if let Some(wait) = self.lower_endpoint_wait(h, cond)? {
            self.saw_wait = true;
            return Ok(wait);
        }
        self.cg.reject_process_status_wait(&self.path, cond)?;
        let c = self.cg.lower_expr(&self.path, cond)?;
        let path = self.path.clone();
        let mut sens = self
            .cg
            .with_dynamic_reads(&path, cond, |cg| cg.collect_read_signals(&path, cond))?;
        sens.extend(self.cg.shared_event_dependencies(cond));
        self.cg.add_strength_reads(&mut sens)?;
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

    fn lower_wait_order(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::WaitOrder {
            events,
            if_true,
            if_false,
        }) = self.cg.kind(h)
        else {
            return Err(format!("malformed wait_order in `{}`", self.path));
        };
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
        // SV 15.5.4: "If the fail statement is not specified, a
        // failure generates a run-time error." The error ends the
        // run at once, without final procedures (S14-D6).
        let failure = match if_false {
            Some(body) => self.lower_stmt(*body)?,
            None => vec![IrStmt::Severity {
                level: crate::sim::ir::IrSeverityLevel::Fatal,
                fmt: "\"wait_order: events triggered out of order\"".to_owned(),
                args: Vec::new(),
                scope: self.format_scope(h),
                location: self.finish_location(h),
                fatal_finish_number: Some(0),
                origin: crate::sim::ir::IrSeverityOrigin::RuntimeFailure,
                skip_finals: true,
            }],
        };
        self.saw_wait = true;
        Ok(vec![IrStmt::WaitOrder {
            events,
            success,
            failure,
        }])
    }

    fn lower_return_statement(&mut self, value: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        if let Some(statements) = self.lower_randsequence_return(value)? {
            return Ok(statements);
        }
        let statement = self.lower_return(value)?;
        match self.coverage_end_exit.clone() {
            Some(exit) => {
                self.coverage_end_exit_used = true;
                Ok(vec![self.return_to_coverage_exit(statement, &exit)?])
            }
            None => Ok(vec![statement]),
        }
    }

    fn lower_super_constructor(&mut self, constructor: NodeId) -> Result<Vec<IrStmt>, String> {
        let (name, is_task, callee) = match self.cg.kind(constructor) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => (name.clone(), *is_task, *callee),
            _ => return Err("super constructor edge is not a function call".to_owned()),
        };
        let mut statements = vec![self.lower_task_call(constructor, &name, is_task, callee)?];
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

    fn lower_method_statement(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            callee,
            ..
        } = self.cg.kind(h)
        else {
            return Err(format!("malformed method call in `{}`", self.path));
        };
        if (matches!(name.as_str(), "suspend" | "await")
            && self.cg.is_process_value(&self.path, *receiver))
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
            let is_task = matches!(self.cg.kind(ft), NodeKind::FuncTask { is_task: true, .. });
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

    /// Block on a procedural `expect`, then run its action block in this
    /// process (16.18): the pass statement on success, the else statement on
    /// failure, neither when the evaluation ended without a result. The arms
    /// see the assertion's leading clock for sampled-value functions.
    fn lower_expect_actions(
        &mut self,
        identity: u64,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
        captures: Vec<IrExpr>,
    ) -> Result<Vec<IrStmt>, String> {
        let clock = self.cg.take_expect_action_clock();
        let previous_clock = std::mem::replace(&mut self.cg.sampled_clock, clock);
        let arms = (|| {
            let pass = if_true.map(|arm| self.lower_stmt(arm)).transpose()?;
            let fail = if_false.map(|arm| self.lower_stmt(arm)).transpose()?;
            Ok::<_, String>((pass, fail))
        })();
        self.cg.sampled_clock = previous_clock;
        let (pass, fail) = arms?;
        let mut statements = vec![IrStmt::Expect {
            identity,
            fail_action: fail.is_some(),
            captures,
        }];
        let outcome_is = |value: u64| -> Result<IrExpr, String> {
            let constant = IrConst::packed(vec![value], vec![], vec![], 2, false, None)
                .map_err(|error| error.to_string())?;
            Ok(IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::CaseEq,
                    a: Box::new(IrExpr::new(
                        IrExprKind::RuntimeQuery(IrRuntimeQuery::ExpectOutcome(identity)),
                        2,
                        false,
                        None,
                    )),
                    b: Box::new(IrExpr::new(IrExprKind::Const(constant), 2, false, None)),
                },
                1,
                false,
                None,
            ))
        };
        let fail = match fail {
            Some(body) if !body.is_empty() => Some(vec![IrStmt::If {
                cond: outcome_is(2)?,
                then_: body,
                els: None,
                check: IrUniquePriorityCheck::None,
            }]),
            _ => None,
        };
        match pass {
            Some(body) if !body.is_empty() => statements.push(IrStmt::If {
                cond: outcome_is(1)?,
                then_: body,
                els: fail,
                check: IrUniquePriorityCheck::None,
            }),
            _ => statements.extend(fail.into_iter().flatten()),
        }
        Ok(statements)
    }
}

fn locate_suspensions(statements: &mut [IrStmt], origin: &crate::sim::semantic::Origin) {
    for statement in statements {
        match statement {
            IrStmt::Located { .. } => continue,
            IrStmt::Block(body)
            | IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::Forever { body }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::WaitEventTriggered { body, .. }
            | IrStmt::ActivationScope { body, .. } => locate_suspensions(body, origin),
            IrStmt::If { then_, els, .. } => {
                locate_suspensions(then_, origin);
                if let Some(els) = els {
                    locate_suspensions(els, origin);
                }
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                locate_suspensions(init, origin);
                locate_suspensions(incr, origin);
                locate_suspensions(body, origin);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    locate_suspensions(&mut item.body, origin);
                }
            }
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                locate_suspensions(success, origin);
                locate_suspensions(failure, origin);
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(body) = if_true {
                    locate_suspensions(body, origin);
                }
                if let Some(body) = if_false {
                    locate_suspensions(body, origin);
                }
            }
            _ => {}
        }
        if matches!(
            statement,
            IrStmt::Delay { .. }
                | IrStmt::WaitEvents { .. }
                | IrStmt::WaitAny { .. }
                | IrStmt::WaitCond { .. }
                | IrStmt::WaitEventTriggered { .. }
                | IrStmt::WaitOrder { .. }
                | IrStmt::ClockingCycleWait { .. }
                | IrStmt::Fork { .. }
                | IrStmt::CapturedFork { .. }
                | IrStmt::WaitFork
                | IrStmt::Expect { .. }
                | IrStmt::StopControl { .. }
                | IrStmt::Call(_)
        ) || matches!(
            statement,
            IrStmt::Object(object) if matches!(
                **object,
                IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Suspend,
                    ..
                } | IrObjectStmt::ProcessAwait(_)
                    | IrObjectStmt::SemaphoreGet(..)
                    | IrObjectStmt::MailboxPut(..)
                    | IrObjectStmt::MailboxPutLocal(..)
                    | IrObjectStmt::MailboxGet(..)
                    | IrObjectStmt::MailboxGetLocal(..)
            )
        ) {
            *statement = std::mem::replace(statement, IrStmt::Nop).with_origin(origin.clone());
        }
    }
}
