//! Procedural statement lowering through the shared emission context.

use super::*;

fn loop_index_expr(value: i32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![value as u32 as u64],
            x: vec![0],
            z: vec![0],
            width: 32,
            signed: true,
            real: None,
            fill: None,
        }),
        32,
        true,
        None,
    )
}

impl<'c, 'a> EmitCtx<'c, 'a> {
    /// Build an emission context and record `func`/`depth_arg`/`inst` on the
    /// codegen, where expression and LHS resolution reads them (refs inside
    /// nested operations recurse through `Codegen::emit_expr`, which has no
    /// access to the `EmitCtx` itself).
    pub(super) fn new(
        cg: &'c mut Codegen<'a>,
        path: String,
        inst: NodeId,
        depth_arg: &str,
        func: Option<FuncCtx>,
        inline: Option<InlineCtx>,
        in_final: bool,
    ) -> EmitCtx<'c, 'a> {
        cg.func = func.clone();
        cg.depth_arg = depth_arg.to_string();
        cg.inst = inst;
        EmitCtx {
            cg,
            path,
            saw_wait: false,
            inst,
            depth_arg: depth_arg.to_string(),
            func,
            inline,
            pre_fns: Vec::new(),
            ctrl: Vec::new(),
            label_seq: 0,
            in_final,
        }
    }

    /// Allocate a fresh control-flow label (`_xb3`, `_bk7`, `_ct9`).  The
    /// sequence is per emitted C function (see [`EmitCtx::ctrl`]); distinct
    /// tags cannot collide with each other or with the inline-task done
    /// labels (`_id<node>`).
    fn new_label(&mut self, tag: &str) -> String {
        self.label_seq += 1;
        format!("_{tag}{}", self.label_seq)
    }

    /// Lower a loop body under a break/continue scope.  The continue label
    /// is appended to the body's END — for every loop shape that lands on
    /// the next-iteration point (for: the increment step; while/repeat/
    /// forever: the back-edge condition test).  Returns the lowered body and
    /// the trailing break label when any `break` used it.
    fn lower_loop_body(
        &mut self,
        body_node: NodeId,
    ) -> Result<(Vec<IrStmt>, Option<IrStmt>), String> {
        let brk = self.new_label("bk");
        let cont = self.new_label("ct");
        self.ctrl.push(CtrlScope::Loop {
            brk,
            brk_used: false,
            cont,
            cont_used: false,
        });
        let mut body = self.lower_stmt(body_node)?;
        match self.ctrl.pop() {
            Some(CtrlScope::Loop {
                brk,
                brk_used,
                cont,
                cont_used,
            }) => {
                if cont_used {
                    body.push(IrStmt::Label(cont));
                }
                if brk_used {
                    Ok((body, Some(IrStmt::Label(brk))))
                } else {
                    Ok((body, None))
                }
            }
            _ => unreachable!("loop scope stack imbalance"),
        }
    }

    /// Lower a SystemVerilog `do body while (cond)` post-test loop.  The
    /// source body runs before the condition on every iteration.  A source
    /// `continue` jumps to the label immediately before that condition, and
    /// both a false condition and a source `break` jump to the label after
    /// the enclosing forever loop.
    fn lower_do_while(
        &mut self,
        cond_node: NodeId,
        body_node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let cond = self.cg.lower_expr(&self.path, cond_node)?;
        let brk = self.new_label("bk");
        let cont = self.new_label("ct");
        self.ctrl.push(CtrlScope::Loop {
            brk: brk.clone(),
            brk_used: false,
            cont: cont.clone(),
            cont_used: false,
        });
        let mut body = self.lower_stmt(body_node)?;
        match self.ctrl.pop() {
            Some(CtrlScope::Loop {
                cont_used,
                brk: actual_brk,
                cont: actual_cont,
                ..
            }) => {
                debug_assert_eq!(actual_brk, brk);
                debug_assert_eq!(actual_cont, cont);
                if cont_used {
                    body.push(IrStmt::Label(cont));
                }
            }
            _ => unreachable!("loop scope stack imbalance"),
        }
        body.push(IrStmt::If {
            cond,
            then_: Vec::new(),
            els: Some(vec![IrStmt::Goto(brk.clone())]),
        });
        Ok(vec![IrStmt::Forever { body }, IrStmt::Label(brk)])
    }

    /// Lower `break;` / `continue;`: an atomic jump to the innermost loop's
    /// break/continue label.  Resolution stops at an inlined-task-body
    /// boundary (a `break` can never target a loop of the CALLER) and
    /// outside a loop this is a syntax-level error that Surelog normally
    /// rejects first; kept as a clean codegen reject.
    fn lower_break_continue(&mut self, is_break: bool) -> Result<Vec<IrStmt>, String> {
        for scope in self.ctrl.iter_mut().rev() {
            match scope {
                CtrlScope::Loop {
                    brk,
                    brk_used,
                    cont,
                    cont_used,
                } => {
                    return Ok(if is_break {
                        *brk_used = true;
                        vec![IrStmt::Goto(brk.clone())]
                    } else {
                        *cont_used = true;
                        vec![IrStmt::Goto(cont.clone())]
                    });
                }
                // Inlined task expansion: its body must resolve jumps within
                // itself only.
                CtrlScope::TaskBody => break,
                CtrlScope::Block { .. } => {}
            }
        }
        Err(format!(
            "`{}` outside a loop in `{}`",
            if is_break { "break" } else { "continue" },
            self.path
        ))
    }

    /// Lower `disable <label>;` (1364-1995 §11) as a same-process jump:
    ///
    /// - target is an enclosing named begin block on the scope stack →
    ///   `goto` its exit label (statements after the disable are skipped);
    /// - target is the task/function currently being INLINED at this site →
    ///   jump to that expansion's done label (= early `return;`);
    /// - target is the plain (non-inlined) function/task whose body is being
    ///   lowered → an early C `return` from it;
    /// - anything else (a block/task of another process, a named fork,
    ///   an outer inlined task) would need cross-coroutine termination and
    ///   is rejected with a clear error instead of mis-lowering.
    fn lower_disable(&mut self, target: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        let Some(t) = target else {
            return Err(format!(
                "cannot resolve the target of `disable` in `{}`: no matching \
                 enclosing named block or task of the same process (cross-\
                 process disables are not supported in v1)",
                self.path
            ));
        };
        for scope in self.ctrl.iter_mut().rev() {
            if let CtrlScope::Block {
                node,
                exit,
                exit_used,
            } = scope
            {
                if *node == t {
                    *exit_used = true;
                    return Ok(vec![IrStmt::Goto(exit.clone())]);
                }
            }
        }
        // Disabling the innermost inlined task equals an early return out of
        // THIS expansion only; outer inlined tasks stay rejected (their done
        // labels belong to other expansions).
        if let Some(inl) = self.inline.as_mut() {
            if inl.def == t {
                inl.used = true;
                return Ok(vec![IrStmt::Goto(inl.done_label.clone())]);
            }
        }
        if let Some(f) = self.func.as_ref() {
            if f.def_node == Some(t) {
                return Ok(vec![IrStmt::Return { value: None }]);
            }
        }
        Err(format!(
            "disable of `{}` in `{}` is not supported in v1: only an \
             enclosing named block of the same process, or the task itself \
             as an early return, can be disabled (cross-process disables, \
             named forks and outer inlined tasks are not supported)",
            self.cg.node(t).name,
            self.path
        ))
    }
}

impl EmitCtx<'_, '_> {
    /// Lower one statement (or construct) into its IR statements.  Mirrors
    /// the pre-IR emitter decision-for-decision: same errors, warnings,
    /// sensitivity sets and wait tracking.
    pub(super) fn lower_stmt(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Begin) => {
                let mut body = Vec::new();
                // A named block is a potential `disable` target: its exit
                // label is allocated before the body lowers so disables
                // inside (including inside inlined task expansions) can
                // reference it.
                let named = !self.cg.node(h).name.is_empty();
                if named {
                    let exit = self.new_label("xb");
                    self.ctrl.push(CtrlScope::Block {
                        node: h,
                        exit,
                        exit_used: false,
                    });
                }
                for s in &self.cg.node(h).children {
                    // Local variable declarations inside the block are hoisted
                    // by the function-local collection; skip them here.
                    if matches!(
                        self.cg.kind(*s),
                        NodeKind::Var { .. } | NodeKind::FuncArg { .. }
                    ) {
                        continue;
                    }
                    body.extend(self.lower_stmt(*s)?);
                }
                if named {
                    match self.ctrl.pop() {
                        Some(CtrlScope::Block {
                            exit, exit_used, ..
                        }) => {
                            if exit_used {
                                body.push(IrStmt::Label(exit));
                            }
                        }
                        _ => unreachable!("block scope stack imbalance"),
                    }
                }
                Ok(vec![IrStmt::Block(body)])
            }
            NodeKind::Stmt(StmtKind::IfElse { cond }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
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
                }])
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking, delay, ..
            }) => match delay {
                None => Ok(vec![self.lower_assignment(h, false)?]),
                Some(IntraControl::Ticks(ticks)) => {
                    if self.in_final {
                        return Err(format!(
                            "intra-assignment delay inside a final block in `{}` is not \
                             allowed (no timing controls in final)",
                            self.path
                        ));
                    }
                    let unit_ps = self.cg.timescale_of_node(h).unit_ps;
                    let scaled = scale_delay_ticks(
                        *ticks,
                        unit_ps,
                        self.cg.design_precision_ps,
                        &self.path,
                    )?;
                    self.lower_delayed_assignment(h, *blocking, scaled)
                }
                Some(IntraControl::Expression(expression)) => {
                    if self.in_final {
                        return Err(format!(
                            "intra-assignment delay inside a final block in `{}` is not \
                             allowed (no timing controls in final)",
                            self.path
                        ));
                    }
                    let scaled = self.cg.procedural_delay_ticks(h, expression)?;
                    self.lower_delayed_assignment(h, *blocking, scaled)
                }
                Some(IntraControl::EventOrRepeat) => Err(format!(
                    "intra-assignment event/repeat control (`@(…)` or \
                     `repeat (n) @(…)`) in `{}` is not supported in v1",
                    self.path
                )),
                Some(IntraControl::UnresolvedDelay) => {
                    let node = self.cg.node(h);
                    let file = node.file.clone().unwrap_or_default();
                    Err(format!(
                        "cannot determine the `#delay` value at {file}:{} \
                         (parameterized delays are not supported in v1)",
                        node.line
                    ))
                }
            },
            NodeKind::Stmt(StmtKind::DelayControl { ticks, expression }) => {
                if self.in_final {
                    return Err(format!(
                        "`#delay` inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "delay `#{}` inside a function/task body in `{}` is not \
                         supported (delay-bearing tasks are inlined at their call \
                         sites)",
                        ticks
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        self.path
                    ));
                }
                let scaled = match (ticks, expression) {
                    (Some(t), _) => {
                        let unit_ps = self.cg.timescale_of_node(h).unit_ps;
                        scale_delay_ticks(*t, unit_ps, self.cg.design_precision_ps, &self.path)?
                    }
                    (None, Some(expression)) => self.cg.procedural_delay_ticks(h, expression)?,
                    (None, None) => {
                        let file = self.cg.node(h).file.clone().unwrap_or_default();
                        let line = self.cg.node(h).line;
                        return Err(format!(
                            "cannot determine the `#delay` value at {file}:{line}"
                        ));
                    }
                };
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
            NodeKind::Stmt(StmtKind::EventControl { .. }) => {
                if self.in_final {
                    return Err(format!(
                        "`@(...)` event control inside a final block in `{}` is not \
                         allowed (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "event control inside a function/task body in `{}` is not \
                         supported (delay-bearing tasks are inlined at their call \
                         sites)",
                        self.path
                    ));
                }
                self.lower_event_control(h)
            }
            NodeKind::Stmt(StmtKind::EventTrigger { target, .. }) => {
                // `-> ev;` (and `->> ev;`, indistinguishable in Surelog v1.86
                // output): wake every current waiter of the event.  A trigger
                // never suspends the process, so it is accepted inside
                // function bodies: IEEE 1800-2009 §13.4.4 explicitly allows
                // non-blocking statements there ("specifically, nonblocking
                // assignments, event triggers, …"), and 1364-2001 §10.3.4(a)
                // bans only statements introduced with #/@/wait.  The
                // `blocking` property is not relied on: Surelog reports 1 for
                // both trigger forms.
                let target = target.ok_or_else(|| {
                    format!(
                        "cannot resolve named event reference `{}` in `{}`",
                        self.cg.node(h).name,
                        self.path
                    )
                })?;
                let idx = self.cg.event_index_of(target, &self.path)?;
                Ok(vec![IrStmt::EventTrigger { ev: idx }])
            }
            NodeKind::Stmt(StmtKind::Case { .. }) => self.lower_case(h),
            NodeKind::Stmt(StmtKind::For { .. }) => self.lower_for(h),
            NodeKind::Stmt(StmtKind::While { cond, body }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
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
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "wait inside a function/task body in `{}` is not supported \
                         (wait-bearing tasks are inlined at their call sites)",
                        self.path
                    ));
                }
                let c = self.cg.lower_expr(&self.path, *cond)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued wait conditions are not supported in `{}`",
                        self.path
                    ));
                }
                let sens = self.cg.collect_read_signals(&self.path, *cond)?;
                self.saw_wait = true;
                // The body is optional (`wait (cond);`); the database walk
                // captures it as a child only when Surelog emits a `vpiStmt`.
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
            NodeKind::Stmt(StmtKind::Force { .. }) => Ok(vec![self.lower_force(h)?]),
            NodeKind::Stmt(StmtKind::Release { .. }) => Ok(vec![self.lower_release(h)?]),
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => self.lower_proc_cont_assign(h),
            NodeKind::Stmt(StmtKind::Deassign { lhs }) => self.lower_deassign(*lhs),
            NodeKind::Stmt(StmtKind::Empty) => Ok(vec![IrStmt::Nop]),
            NodeKind::Stmt(StmtKind::Return { value }) => Ok(vec![self.lower_return(*value)?]),
            NodeKind::Stmt(StmtKind::Fork {
                join_kind,
                branches,
            }) => self.lower_fork(*join_kind, branches),
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
            NodeKind::SysCall { name } => self.lower_sys_call(h, name),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
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
            NodeKind::Stmt(StmtKind::Unsupported { vpi_type }) => {
                let node = self.cg.node(h);
                let file = node.file.as_deref().unwrap_or("<unknown>");
                Err(format!(
                    "unsupported executable statement VPI type {vpi_type} at \
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

    /// Lower an assignment without intra-assignment delay (`force_blocking`
    /// pins blocking semantics for for-loop init/increment statements).
    fn lower_assignment(&mut self, h: NodeId, force_blocking: bool) -> Result<IrStmt, String> {
        let (blocking, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, op, .. }) => (*blocking, *op),
            _ => unreachable!("non-assignment passed to lower_assignment"),
        };
        if matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { delay: Some(_), .. })
        ) {
            // Reached only from the for-loop init/increment path; plain
            // statement assignments are classified in lower_stmt.
            return Err(format!(
                "intra-assignment delay on a for-loop init/increment \
                 assignment in `{}` is not supported",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        let blocking = force_blocking || blocking;
        if !blocking {
            if let Some(local) = self.cg.proc_local_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to inline loop variable `{}` in `{}` is not supported because the update can outlive its lexical storage",
                    self.cg.node(local).name,
                    self.path
                ));
            }
        }
        if self.in_final && !blocking {
            return Err(format!(
                "nonblocking assignment inside a final block in `{}` is not \
                 allowed (final permits function statements only)",
                self.path
            ));
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op.as_raw(), &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        Ok(IrStmt::Assign {
            lhs: lh,
            rhs: rhs_ir,
            nba: !blocking,
        })
    }

    /// Lower the value written by a normal or compound procedural
    /// assignment.  Compound assignments currently require a whole scalar
    /// target, which guarantees the LHS is evaluated once; select and array
    /// targets need index temporaries before they can preserve that rule.
    fn lower_assignment_rhs(
        &mut self,
        lhs_node: NodeId,
        rhs_node: NodeId,
        op: i32,
        lhs: &IrLhs,
    ) -> Result<IrExpr, String> {
        let rhs = self.cg.lower_expr(&self.path, rhs_node)?;
        if op == 0 || op == vpi::vpiAssignmentOp {
            return Ok(rhs);
        }
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
            return Err(format!(
                "compound assignment to a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, lhs_node)?;
        self.lower_compound_expr(op, current, rhs)
    }

    fn lower_compound_expr(&self, op: i32, lhs: IrExpr, rhs: IrExpr) -> Result<IrExpr, String> {
        let real = lhs.is_real() || rhs.is_real();
        let result = match op {
            vpi::vpiAddOp | vpi::vpiSubOp | vpi::vpiMultOp => {
                if real {
                    let op = match op {
                        vpi::vpiAddOp => IrRealBinOp::Add,
                        vpi::vpiSubOp => IrRealBinOp::Sub,
                        _ => IrRealBinOp::Mul,
                    };
                    real_bin_expr(op, lhs, rhs)
                } else {
                    let op = match op {
                        vpi::vpiAddOp => IrBinOp::Add,
                        vpi::vpiSubOp => IrBinOp::Sub,
                        _ => IrBinOp::Mul,
                    };
                    common_bin_expr(op, lhs, rhs)
                }
            }
            vpi::vpiDivOp | vpi::vpiModOp => {
                if real {
                    real_bin_expr(
                        if op == vpi::vpiDivOp {
                            IrRealBinOp::Div
                        } else {
                            IrRealBinOp::Mod
                        },
                        lhs,
                        rhs,
                    )
                } else {
                    common_bin_expr(
                        if op == vpi::vpiDivOp {
                            IrBinOp::Div
                        } else {
                            IrBinOp::Mod
                        },
                        lhs,
                        rhs,
                    )
                }
            }
            vpi::vpiBitAndOp | vpi::vpiBitOrOp | vpi::vpiBitXorOp => {
                if real {
                    return Err(format!(
                        "bitwise compound assignment on a real value in `{}` is not supported",
                        self.path
                    ));
                }
                let op = match op {
                    vpi::vpiBitAndOp => IrBinOp::BitAnd,
                    vpi::vpiBitOrOp => IrBinOp::BitOr,
                    _ => IrBinOp::BitXor,
                };
                common_bin_expr(op, lhs, rhs)
            }
            vpi::vpiLShiftOp | vpi::vpiRShiftOp | vpi::vpiArithLShiftOp | vpi::vpiArithRShiftOp => {
                if real {
                    return Err(format!(
                        "shift compound assignment on a real value in `{}` is not supported",
                        self.path
                    ));
                }
                let width = lhs.width;
                let signed = lhs.signed;
                let op = match op {
                    vpi::vpiLShiftOp => IrBinOp::Shl,
                    vpi::vpiRShiftOp => IrBinOp::Shr,
                    vpi::vpiArithLShiftOp => IrBinOp::Ashl,
                    _ => IrBinOp::Ashr,
                };
                IrExpr::new(
                    IrExprKind::Bin {
                        op,
                        a: Box::new(lhs),
                        b: Box::new(rhs),
                    },
                    width,
                    signed,
                    None,
                )
            }
            other => {
                return Err(format!(
                    "unsupported compound assignment operation {other} in `{}`",
                    self.path
                ))
            }
        };
        Ok(result)
    }

    /// Lower a statement-position pre/post increment or decrement.  Since
    /// the operation's value is discarded in statement position, pre and
    /// post forms have the same blocking-write behavior.  Expression-valued
    /// forms require a side-effecting expression IR and remain unsupported.
    fn lower_inc_dec(&mut self, op: Operation, operands: &[NodeId]) -> Result<IrStmt, String> {
        let operand = match operands {
            [operand] => *operand,
            _ => {
                return Err(format!(
                    "increment/decrement in `{}` must have exactly one operand",
                    self.path
                ))
            }
        };
        let lhs = self.cg.lower_lhs(&self.path, operand)?;
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
            return Err(format!(
                "increment/decrement of a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, operand)?;
        let increment = matches!(op, Operation::PostIncrement | Operation::PreIncrement);
        let rhs = if current.is_real() {
            real_bin_expr(
                if increment {
                    IrRealBinOp::Add
                } else {
                    IrRealBinOp::Sub
                },
                current,
                real_literal_expr(1.0),
            )
        } else {
            let one = IrExpr::new(
                IrExprKind::Const(IrConst {
                    bits: vec![1],
                    x: vec![0],
                    z: vec![0],
                    width: 32,
                    signed: true,
                    real: None,
                    fill: None,
                }),
                32,
                true,
                None,
            );
            common_bin_expr(
                if increment {
                    IrBinOp::Add
                } else {
                    IrBinOp::Sub
                },
                current,
                one,
            )
        };
        let rhs = apply_lhs_assignment_context(&self.cg.model, &lhs, rhs);
        Ok(IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        })
    }

    /// Lower an intra-assignment-delayed assignment (`lhs = #N rhs;`,
    /// `lhs <= #N rhs;`; LRM 1364-1995 §9.7.4): the RHS is evaluated once,
    /// immediately, into a temp; the process suspends N ticks; then the LHS
    /// is updated from the temp — as a blocking write, or recorded for the
    /// delayed step's NBA region.  `#0` suspends through the runtime's
    /// zero-delay (inactive-region) wait.
    fn lower_delayed_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        scaled_ticks: u64,
    ) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() && self.inline.is_none() {
            return Err(format!(
                "delay inside a function/task body in `{}` is not supported \
                 (delay-bearing tasks are inlined at their call sites)",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_delayed_assignment"),
        };
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op.as_raw(), &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        if rhs_ir.is_real() {
            return Err(format!(
                "real-valued intra-assignment delays are not supported in `{}`",
                self.path
            ));
        }
        self.saw_wait = true;
        // The temp name is unique per assignment node; each site's Block keeps
        // re-declarations (loops, repeated task inlining) out of one C scope.
        let tmp = format!("_t{}", h.0);
        let (w, s) = (rhs_ir.width, rhs_ir.signed);
        Ok(vec![IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: tmp.clone(),
                width: w,
                signed: s,
                two_state: false,
                init: Some(Box::new(rhs_ir)),
            },
            IrStmt::Delay {
                ticks: scaled_ticks,
            },
            IrStmt::Assign {
                lhs: lh,
                rhs: IrExpr::new(IrExprKind::LocalRead(tmp), w, s, None),
                nba: !blocking,
            },
        ])])
    }

    /// Lower `@(…)`: explicit edge/any specs become ONE atomic
    /// `llg_wait_any_events`; implicit sensitivity waits on the body's read
    /// set.
    fn lower_event_control(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (specs, implicit, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit,
                body,
            }) => (specs, *implicit, *body),
            _ => unreachable!("non-event-control passed to lower_event_control"),
        };
        let body = body.ok_or_else(|| "event_control without body".to_string())?;
        let spec_pairs = self.lower_event_specs(specs)?;
        let wait = if implicit || spec_pairs.is_empty() {
            // @* / always_comb without explicit sensitivity, or a condition
            // that produced no specs: wait on the body's read set.
            IrStmt::WaitAny {
                sens: self.cg.collect_read_signals(&self.path, body)?,
            }
        } else {
            IrStmt::WaitEvents { specs: spec_pairs }
        };
        self.saw_wait = true;
        let mut out = vec![wait];
        // The body may be a `Stmt(Empty)` placeholder for a bare
        // `@(posedge clk);`; skip it.
        if !matches!(self.cg.kind(body), NodeKind::Stmt(StmtKind::Empty)) {
            out.extend(self.lower_stmt(body)?);
        }
        Ok(out)
    }

    /// Resolve the captured event specs into atomic wait sources: signal
    /// globals with edges, plus named events.  A mixed list stays ONE
    /// `WaitEvents` statement (one runtime call), so a trigger can never be
    /// lost between two separate waits.
    fn lower_event_specs(
        &mut self,
        specs: &[EventSpec],
    ) -> Result<Vec<(IrWaitSrc, IrEdge)>, String> {
        let mut out = Vec::new();
        for s in specs {
            match s {
                EventSpec::Edge { sig, posedge } => {
                    // Edge controls on named events are rejected cleanly
                    // (v1): an event has no value, so posedge/negedge have no
                    // meaning (LRM 1364-1995 §9.7.3 note).
                    if let Some(ev) = self.cg.event_target_of(*sig) {
                        let name = self.cg.node(ev).name.clone();
                        return Err(format!(
                            "edge control on a named event is not supported in v1 \
                             (`{name}` in `{}`)",
                            self.path
                        ));
                    }
                    let (name, info) = self.cg.resolve_signal_id(&self.path, *sig)?;
                    if info.real {
                        return Err(format!(
                            "real-valued signals cannot be event-controlled in `{}`",
                            self.path
                        ));
                    }
                    out.push((
                        IrWaitSrc::Sig(name),
                        if *posedge {
                            IrEdge::Posedge
                        } else {
                            IrEdge::Negedge
                        },
                    ));
                }
                EventSpec::AnyChange { sig } => {
                    if let Some(ev) = self.cg.event_target_of(*sig) {
                        let idx = self.cg.event_index_of(ev, &self.path)?;
                        out.push((IrWaitSrc::Event(idx), IrEdge::Any));
                        continue;
                    }
                    let (name, info) = self.cg.resolve_signal_id(&self.path, *sig)?;
                    if info.real {
                        return Err(format!(
                            "real-valued signals cannot be event-controlled in `{}`",
                            self.path
                        ));
                    }
                    out.push((IrWaitSrc::Sig(name), IrEdge::Any));
                }
                EventSpec::Named(ev) => {
                    // `@(ev)` — wait on the event trigger itself; events are
                    // edge-triggered (a trigger before the wait does not
                    // latch).
                    let idx = self.cg.event_index_of(*ev, &self.path)?;
                    out.push((IrWaitSrc::Event(idx), IrEdge::Any));
                }
            }
        }
        Ok(out)
    }

    fn lower_case(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (case_type, items) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Case { case_type, items }) => (*case_type, items),
            _ => unreachable!("non-case passed to lower_case"),
        };
        // VPI case subtypes (vpi_user.h): vpiCaseExact=1 (case), vpiCaseX=2
        // (casex), vpiCaseZ=3 (casez).  casez/casex keep wildcard matching
        // per LRM 12.5.1 instead of degrading to exact equality.
        let kind = match case_type {
            DbCaseKind::Exact => IrCaseKind::Exact,
            DbCaseKind::X => IrCaseKind::Casex,
            DbCaseKind::Z => IrCaseKind::Casez,
            other => return Err(format!("unsupported case type {other} in `{}`", self.path)),
        };
        let sel = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "case without selector".to_string())?;
        let mut sel_ir = self.cg.lower_expr(&self.path, sel)?;
        if sel_ir.is_real() {
            return Err(format!(
                "real-valued case selectors are not supported in `{}`",
                self.path
            ));
        }
        if case_type == DbCaseKind::Exact && self.is_case_inside(items) {
            return self.lower_case_inside(items, sel_ir);
        }
        let mut ir_items = Vec::with_capacity(items.len());
        let mut common_width = sel_ir.width;
        let mut common_signed = sel_ir.signed;
        for item in items.iter() {
            let mut exprs = Vec::with_capacity(item.exprs.len());
            for e in &item.exprs {
                let c = self.cg.lower_expr(&self.path, *e)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued case items are not supported in `{}`",
                        self.path
                    ));
                }
                common_width = common_width.max(c.width);
                common_signed &= c.signed;
                exprs.push(c);
            }
            let mut body = Vec::new();
            if let Some(s) = item.body {
                body = self.lower_stmt(s)?;
            }
            ir_items.push(IrCaseItem { exprs, body });
        }
        // Ordinary case/casez/casex operands share one context-determined
        // maximum width. This is especially visible for an unbased unsized
        // fill selector: case ('1) with four- and eight-bit items compares an
        // eight-bit all-ones value against every item (LRM 1800-2009 §12.5).
        sel_ir = checked_operand_with_context(
            sel_ir,
            common_width,
            common_signed,
            &self.path,
            "case comparison context",
        )?;
        for item in &mut ir_items {
            for expr in &mut item.exprs {
                *expr = checked_operand_with_context(
                    expr.clone(),
                    common_width,
                    common_signed,
                    &self.path,
                    "case comparison context",
                )?;
            }
        }
        Ok(vec![IrStmt::Case {
            sel: sel_ir,
            kind,
            items: ir_items,
        }])
    }

    /// Surelog represents `case (selector) inside` as an exact case whose
    /// non-default items each contain one `vpiInsideOp`. Every operand of that
    /// wrapper is a `vpiListOp`: one child for a wildcard value, two for an
    /// inclusive range. Requiring that full shape avoids confusing an
    /// ordinary case item whose expression happens to use the `inside`
    /// operator with a case-inside statement.
    fn is_case_inside(&self, items: &[crate::core::db::CaseItem]) -> bool {
        let mut saw_item = false;
        for item in items {
            if item.exprs.is_empty() {
                continue;
            }
            saw_item = true;
            let [expr] = item.exprs.as_slice() else {
                return false;
            };
            let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = self.cg.kind(*expr)
            else {
                return false;
            };
            if *op != Operation::Inside
                || operands.is_empty()
                || operands.iter().any(|operand| {
                    !matches!(
                        self.cg.kind(*operand),
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::List,
                            operands,
                            ..
                        }) if matches!(operands.len(), 1 | 2)
                    )
                })
            {
                return false;
            }
        }
        saw_item
    }

    fn lower_case_inside(
        &mut self,
        items: &[crate::core::db::CaseItem],
        selector: IrExpr,
    ) -> Result<Vec<IrStmt>, String> {
        let selector_name = self.new_label("ci");
        let selector_width = selector.width;
        let selector_signed = selector.signed;
        let selector_read = IrExpr::new(
            IrExprKind::LocalRead(selector_name.clone()),
            selector_width,
            selector_signed,
            None,
        );
        let mut branches = Vec::new();
        let mut default = None;
        for item in items {
            let body = match item.body {
                Some(stmt) => self.lower_stmt(stmt)?,
                None => Vec::new(),
            };
            if item.exprs.is_empty() {
                if default.replace(body).is_some() {
                    return Err(format!(
                        "case inside has multiple default items in `{}`",
                        self.path
                    ));
                }
                continue;
            }

            let wrapper = item.exprs[0];
            let operands = match self.cg.kind(wrapper) {
                NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands.clone(),
                _ => unreachable!("case-inside shape checked before lowering"),
            };
            let mut condition = None;
            for operand in operands {
                let members = match self.cg.kind(operand) {
                    NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands.clone(),
                    _ => unreachable!("case-inside list shape checked before lowering"),
                };
                let matched = match members.as_slice() {
                    [value] => {
                        let value = self.cg.lower_expr(&self.path, *value)?;
                        if value.is_real() {
                            return Err(format!(
                                "real-valued case-inside items are not supported in `{}`",
                                self.path
                            ));
                        }
                        wildcard_case_match(&self.path, selector_read.clone(), value)?
                    }
                    [low, high] => {
                        let low = self.cg.lower_expr(&self.path, *low)?;
                        let high = self.cg.lower_expr(&self.path, *high)?;
                        if low.is_real() || high.is_real() {
                            return Err(format!(
                                "real-valued case-inside ranges are not supported in `{}`",
                                self.path
                            ));
                        }
                        case_inside_range_match(&self.path, selector_read.clone(), low, high)?
                    }
                    _ => unreachable!("case-inside list arity checked before lowering"),
                };
                condition = Some(match condition {
                    Some(previous) => cmp_expr_ir(IrBinOp::LogOr, previous, matched),
                    None => matched,
                });
            }
            branches.push((
                condition.expect("case-inside wrapper has at least one operand"),
                body,
            ));
        }

        let mut tail = default;
        for (cond, then_) in branches.into_iter().rev() {
            tail = Some(vec![IrStmt::If {
                cond,
                then_,
                els: tail,
            }]);
        }
        let mut lowered = vec![IrStmt::DeclLocal {
            name: selector_name,
            width: selector_width,
            signed: selector_signed,
            two_state: false,
            init: Some(Box::new(selector)),
        }];
        lowered.extend(tail.unwrap_or_default());
        Ok(lowered)
    }

    fn lower_for(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        // Verified against UHDM for_stmt.h: vpiForInitStmt (75) = init stmt(s),
        // vpiCondition (71) = condition, vpiForIncStmt (74) = increment stmt(s),
        // vpiStmt (104) = body.
        let (vars, init, cond, incr, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::For {
                vars,
                init,
                cond,
                incr,
                body,
            }) => (vars.clone(), init.clone(), *cond, incr.clone(), *body),
            _ => unreachable!("non-for passed to lower_for"),
        };
        let mut declarations = Vec::with_capacity(vars.len());
        for variable in vars {
            let info = self.cg.collect_loop_var(&self.path, variable)?;
            declarations.push(IrStmt::DeclLocal {
                name: info.c_name,
                width: info.width,
                signed: info.signed,
                two_state: info.two_state,
                init: None,
            });
        }
        let mut init_stmts = Vec::with_capacity(init.len());
        for s in &init {
            match self.cg.kind(*s) {
                NodeKind::Stmt(StmtKind::Assign { .. }) => {
                    init_stmts.push(self.lower_assignment(*s, true)?);
                }
                other => {
                    return Err(format!(
                        "unsupported for-loop initializer in `{}` (node kind {other:?})",
                        self.path
                    ))
                }
            }
        }
        let cond_ir = self.cg.lower_expr(&self.path, cond)?;
        let (body_stmts, brk) = self.lower_loop_body(body)?;
        let mut incr_stmts = Vec::with_capacity(incr.len());
        for s in &incr {
            match self.cg.kind(*s) {
                NodeKind::Stmt(StmtKind::Assign { .. }) => {
                    incr_stmts.push(self.lower_assignment(*s, true)?);
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
                    incr_stmts.push(self.lower_inc_dec(*op, operands)?)
                }
                other => {
                    return Err(format!(
                        "unsupported for-loop increment in `{}` (node kind {other:?})",
                        self.path
                    ))
                }
            }
        }
        // `continue` must run the increment before the condition test: its
        // label sits at the END of the body, and the emitter renders the
        // body BEFORE the incr statements inside the C `for`.
        let mut loop_stmts = vec![IrStmt::For {
            init: init_stmts,
            cond: cond_ir,
            incr: incr_stmts,
            body: body_stmts,
        }];
        // The break label trails the whole construct.
        loop_stmts.extend(brk);
        if declarations.is_empty() {
            Ok(loop_stmts)
        } else {
            declarations.extend(loop_stmts);
            Ok(vec![IrStmt::Block(declarations)])
        }
    }

    fn lower_foreach(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (array, vars, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Foreach { array, vars, body }) => {
                (*array, vars.clone(), *body)
            }
            _ => unreachable!("non-foreach passed to lower_foreach"),
        };
        let array = array.ok_or_else(|| {
            format!(
                "cannot resolve the array iterated by `foreach` in `{}`",
                self.path
            )
        })?;
        let array_info = self.cg.array_globals.get(&array).cloned().ok_or_else(|| {
            format!(
                "`foreach` target `{}` in `{}` is not a fixed unpacked array",
                self.cg.node(array).name,
                self.path
            )
        })?;
        if vars.len() != array_info.dims.len() {
            return Err(format!(
                "`foreach` over {}-dimensional array `{}` in `{}` requires one explicit index variable per dimension",
                array_info.dims.len(),
                self.cg.node(array).name,
                self.path
            ));
        }

        let mut declarations = Vec::with_capacity(vars.len());
        let mut locals = Vec::with_capacity(vars.len());
        for variable in vars {
            let info = self.cg.collect_loop_var(&self.path, variable)?;
            declarations.push(IrStmt::DeclLocal {
                name: info.c_name.clone(),
                width: info.width,
                signed: info.signed,
                two_state: info.two_state,
                init: None,
            });
            locals.push(info);
        }

        let (source_body, brk) = self.lower_loop_body(body)?;
        let mut nested = source_body;
        for (local, (left, right)) in locals.iter().zip(&array_info.dims).rev() {
            let read = || {
                IrExpr::new(
                    IrExprKind::LocalRead(local.c_name.clone()),
                    local.width,
                    local.signed,
                    None,
                )
            };
            let init = IrStmt::Assign {
                lhs: IrLhs::WholeRef {
                    addr: format!("&{}", local.c_name),
                    width: local.width,
                    signed: local.signed,
                    two_state: local.two_state,
                },
                rhs: IrExpr::resize_to(loop_index_expr(*left), local.width, local.signed),
                nba: false,
            };
            let increasing = left <= right;
            let done = self.new_label("fe");
            let at_endpoint = common_bin_expr(IrBinOp::Eq, read(), loop_index_expr(*right));
            let next = common_bin_expr(
                if increasing {
                    IrBinOp::Add
                } else {
                    IrBinOp::Sub
                },
                read(),
                loop_index_expr(1),
            );
            let incr = IrStmt::Assign {
                lhs: IrLhs::WholeRef {
                    addr: format!("&{}", local.c_name),
                    width: local.width,
                    signed: local.signed,
                    two_state: local.two_state,
                },
                rhs: IrExpr::resize_to(next, local.width, local.signed),
                nba: false,
            };
            nested.push(IrStmt::If {
                cond: at_endpoint,
                then_: vec![IrStmt::Goto(done.clone())],
                els: None,
            });
            nested.push(incr);
            nested = vec![
                IrStmt::Block(vec![init, IrStmt::Forever { body: nested }]),
                IrStmt::Label(done),
            ];
        }
        nested.extend(brk);
        declarations.extend(nested);
        Ok(vec![IrStmt::Block(declarations)])
    }

    /// Lower one `fork … join` site.  Each branch becomes its own coroutine
    /// function attached to the enclosing process's pre-functions; nested
    /// constructs inside a branch append their own pre-functions first,
    /// mirroring the pre-IR emission order.
    ///
    /// Fork inside a function/task body is rejected: the branch functions are
    /// standalone coroutines with no access to the enclosing function's
    /// formals/locals.
    fn lower_fork(
        &mut self,
        join_kind: DbJoinKind,
        branches: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        if let Some(local) = branches
            .iter()
            .find_map(|branch| self.cg.nested_proc_local_ref(*branch))
        {
            return Err(format!(
                "fork branch capture of inline loop variable `{}` in `{}` is not supported",
                self.cg.node(local).name,
                self.path
            ));
        }
        if self.func.is_some() {
            return Err(format!(
                "fork/join inside a function/task body in `{}` is not supported in v1",
                self.path
            ));
        }
        if self.in_final {
            // A join suspends the process; a final block may not suspend
            // (LRM 1800-2005 §10.7).
            return Err(format!(
                "fork/join inside a final block in `{}` is not allowed \
                 (no timing controls or waits in final)",
                self.path
            ));
        }
        let join = match join_kind {
            DbJoinKind::All => IrJoinKind::Join,
            DbJoinKind::None => IrJoinKind::None,
            DbJoinKind::Any => IrJoinKind::Any,
            k => return Err(format!("unsupported fork join type {k} in `{}`", self.path)),
        };
        let mut names = Vec::with_capacity(branches.len());
        for (k, branch) in branches.iter().enumerate() {
            let fn_name = format!("{}_b{}", self.cg.new_fn_name(&self.path, "fork"), k);
            // Branches live in the same instance scope: same path, same
            // owning instance; refs resolve to the instance globals.
            let mut bctx = EmitCtx::new(
                self.cg,
                self.path.clone(),
                self.inst,
                "0",
                None,
                None,
                false,
            );
            let body = bctx.lower_stmt(*branch)?;
            self.pre_fns.append(&mut bctx.pre_fns);
            self.pre_fns.push(crate::sim::ir::IrPreFn::Branch {
                c_name: fn_name.clone(),
                body,
            });
            names.push((fn_name, format!("{}.br{k}", self.path)));
        }
        // A fork is a wait even for join_none: a wait-free `always` containing
        // `fork … join_none` must not be wrapped as a comb process (its
        // branches are child coroutines, not combinational re-evaluation).
        self.saw_wait = true;
        Ok(vec![IrStmt::Fork {
            join_kind: join,
            branches: names,
        }])
    }

    /// Lower `force lhs = rhs;` — override a whole signal's value until
    /// released.  While forced, the runtime drops procedural writes to the
    /// signal; only whole-signal targets are supported (no selects/arrays, no
    /// collapsed-net members).
    fn lower_force(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "force without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "force without RHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let (sig_idx, width, signed) = match &lh {
            IrLhs::Whole(idx) => {
                let sig = self.cg.model.signal(*idx);
                match sig.ty {
                    IrType::Real { .. } => {
                        return Err(
                            "force/release of real-valued signal is not supported".to_string()
                        );
                    }
                    t => {
                        if sig.net_driver.is_none() {
                            (*idx, t.width(), t.signed())
                        } else {
                            return Err(format!(
                                "force on `{}` in `{}` is not supported (whole signals only)",
                                self.cg.node(lhs).name,
                                self.path
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(format!(
                    "force on `{}` in `{}` is not supported (whole signals only)",
                    self.cg.node(lhs).name,
                    self.path
                ))
            }
        };
        let rhs_ir = apply_assignment_expression_width(self.cg.lower_expr(&self.path, rhs)?, width);
        // Force writes an assignment into the signal: value-preserving
        // RHS→target conversion (LRM §10.7).
        let value = ir_to_vector(rhs_ir, width, signed)?;
        Ok(IrStmt::Force {
            sig: sig_idx,
            value,
        })
    }

    /// Lower `release lhs;` — cancel a procedural force on a whole signal.
    fn lower_release(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "release without LHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        match &lh {
            IrLhs::Whole(idx) => {
                let sig = self.cg.model.signal(*idx);
                if matches!(sig.ty, IrType::Real { .. }) {
                    return Err("force/release of real-valued signal is not supported".to_string());
                }
                if sig.net_driver.is_some() {
                    return Err(format!(
                        "release on `{}` in `{}` is not supported (whole signals only)",
                        self.cg.node(lhs).name,
                        self.path
                    ));
                }
                Ok(IrStmt::Release { sig: *idx })
            }
            _ => Err(format!(
                "release on `{}` in `{}` is not supported (whole signals only)",
                self.cg.node(lhs).name,
                self.path
            )),
        }
    }

    /// Resolve the target of a procedural continuous `assign` / `deassign`.
    /// Variables only (LRM 1364-1995 §9.4): nets, selects/part-selects/
    /// array elements, hierarchical paths and (v1 scope) real variables are
    /// rejected cleanly.  Returns the signal's IR index plus its info.
    fn pca_target(&mut self, lhs: NodeId, stmt: &str) -> Result<(usize, SignalInfo), String> {
        if matches!(self.cg.kind(lhs), NodeKind::Expr(ExprKind::HierPath { .. })) {
            return Err(format!(
                "procedural continuous `{stmt}` on a hierarchical target in `{}` is not \
                 supported (variables of the current scope only)",
                self.path
            ));
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let sig_idx = match &lh {
            IrLhs::Whole(idx) => *idx,
            _ => {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables only)",
                    self.path
                ));
            }
        };
        // Net targets: the elaborated ref normally binds the declaration, so
        // the net/var distinction is read straight off the arena node.
        // Surelog v1.86 quirk: a module-level `reg` is captured as
        // NodeKind::Net with net_type vpiReg (48) — that IS a variable.
        if let NodeKind::Expr(ExprKind::Ref { target: Some(t) }) = self.cg.kind(lhs) {
            if let NodeKind::Net { net_type, .. } = self.cg.kind(*t) {
                if *net_type != NetType::Reg {
                    return Err(format!(
                        "procedural continuous `{stmt}` on net `{}` in `{}` is not supported \
                         (variables only)",
                        self.cg.node(*t).name,
                        self.path
                    ));
                }
            }
        }
        let info = self
            .cg
            .signals
            .iter()
            .find(|i| i.ir == sig_idx)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "cannot collect `{}` in `{}` as a procedural continuous assignment \
                     target (whole variables of the current scope only)",
                    self.cg.node(lhs).name,
                    self.path
                )
            })?;
        if info.real {
            return Err(format!(
                "procedural continuous `{stmt}` on real-valued variable in `{}` is not \
                 supported in v1",
                self.path
            ));
        }
        Ok((sig_idx, info))
    }

    /// Pre-scan phase 1 for one process body: resolve each collected
    /// ProcContAssign target and allocate its site (enable signal) without
    /// lowering anything.  A variable that already has a site is rejected
    /// HERE — order-independently, whichever statement comes second in the
    /// db child order.
    pub(super) fn claim_pca_sites(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        for n in nodes {
            let lhs = self
                .cg
                .node(*n)
                .children
                .first()
                .copied()
                .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
            let (sig_idx, _) = self.pca_target(lhs, "assign")?;
            if self.cg.pca_sites.contains_key(&sig_idx) {
                return Err(self.cg.pca_multi_site_err(lhs, &self.path));
            }
            let en = self.cg.new_pca_enable(&self.path);
            self.cg.pca_sites.insert(
                sig_idx,
                PcaSite {
                    en,
                    guarded_by: None,
                },
            );
        }
        Ok(())
    }

    /// Lower `assign <variable> = expr;` (procedural continuous assignment,
    /// LRM 1364-1995 §9.4).  Decided model, one process per SITE:
    ///
    /// ```text
    /// for (;;) {                       // IrShape::Loop guard process
    ///     wait_any(rhs_reads ∪ en);    // en changes wake the guard too
    ///     if (en) write(lhs, rhs_now); // disabled wakes skip silently
    /// }
    /// ```
    ///
    /// The statement itself sets `en = 1` AND performs an immediate blocking
    /// write (so the value updates in the same delta); `deassign` clears
    /// `en` only — the variable KEEPS its last value (LRM).  Both writes go
    /// through the normal `llg_ba` path, so an active `force` keeps
    /// overriding the PCA (LRM 10.6 interplay).
    ///
    /// NBA-vs-PCA interplay: while a site is enabled, ordinary procedural
    /// writes to the variable — blocking AND non-blocking — still take
    /// effect immediately/as usual; the guard never wakes on changes of the
    /// TARGET itself, so such a write survives until the next wake
    /// triggered by an RHS-read or the enable, which re-drives the variable
    /// from the CURRENT rhs.
    ///
    /// Sites are pre-allocated by [`Codegen::prescan_pca_sites`] before any
    /// body lowers; this method only CLAIMS the pre-allocated site by
    /// materializing its guard process.
    fn lower_proc_cont_assign(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "procedural continuous assignment without RHS".to_string())?;
        let (sig_idx, info) = self.pca_target(lhs, "assign")?;

        // The dedicated guard process: waits on the RHS read set ∪ {en} and
        // re-writes the CURRENT rhs whenever it wakes while enabled.
        let mut sens = Vec::new();
        let mut seen = HashSet::new();
        let mut visited = HashSet::new();
        self.cg
            .walk_read_signals(&self.path, rhs, &mut seen, &mut visited, &mut sens)?;
        let rhs_reads_real = sens
            .iter()
            .any(|name| self.cg.signals.iter().any(|i| i.real && &i.global == name));
        if rhs_reads_real {
            return Err(format!(
                "real-valued signals in the RHS of a procedural continuous \
                 assignment in `{}` are not supported in v1",
                self.path
            ));
        }
        let rhs_ir =
            apply_assignment_expression_width(self.cg.lower_expr(&self.path, rhs)?, info.width);
        // The guard re-writes an assignment into the variable:
        // value-preserving RHS→target conversion (LRM §10.7).
        let value = ir_to_vector(rhs_ir, info.width, info.signed)?;
        let one = IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![1],
                x: vec![0],
                z: vec![0],
                width: 1,
                signed: false,
                real: None,
                fill: None,
            }),
            1,
            false,
            None,
        );

        // Site bookkeeping.  Sites exist for every process-body statement
        // already (pre-scan); a missing entry is only reachable from trees
        // the pre-scan does not walk (defensive fallback with identical
        // semantics).
        let existing = self
            .cg
            .pca_sites
            .get(&sig_idx)
            .map(|s| (s.en, s.guarded_by));
        let en_ir = match existing {
            Some((en, Some(prev))) => {
                if prev != h {
                    return Err(self.cg.pca_multi_site_err(lhs, &self.path));
                }
                // The same statement lowered again (a delay-bearing task
                // body inlined at several call sites): site and guard both
                // exist — only re-execute enable + immediate write.
                return Ok(vec![
                    IrStmt::Assign {
                        lhs: IrLhs::Whole(en),
                        rhs: one,
                        nba: false,
                    },
                    IrStmt::Assign {
                        lhs: IrLhs::Whole(sig_idx),
                        rhs: value,
                        nba: false,
                    },
                ]);
            }
            Some((en, None)) => {
                if let Some(site) = self.cg.pca_sites.get_mut(&sig_idx) {
                    site.guarded_by = Some(h);
                }
                en
            }
            None => {
                let en = self.cg.new_pca_enable(&self.path);
                self.cg.pca_sites.insert(
                    sig_idx,
                    PcaSite {
                        en,
                        guarded_by: Some(h),
                    },
                );
                en
            }
        };
        let en_global = self.cg.model.signals[en_ir].c_name.clone();
        if !sens.contains(&en_global) {
            sens.push(en_global);
        }
        let guard_body = vec![
            IrStmt::WaitAny { sens },
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(en_ir), 1, false, None),
                then_: vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(sig_idx),
                    rhs: value.clone(),
                    nba: false,
                }],
                els: None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "pca");
        self.cg.model.processes.push(IrProcess {
            c_name: guard_name,
            label: format!("{}.pca", self.path),
            shape: IrShape::Loop,
            pre_fns: Vec::new(),
            body: guard_body,
        });

        // Statement execution: enable, then the immediate blocking write
        // (dropped by the runtime while the target is forced).
        Ok(vec![
            IrStmt::Assign {
                lhs: IrLhs::Whole(en_ir),
                rhs: one,
                nba: false,
            },
            IrStmt::Assign {
                lhs: IrLhs::Whole(sig_idx),
                rhs: value,
                nba: false,
            },
        ])
    }

    /// Lower `deassign <variable>;` — cancel the procedural continuous
    /// assignment by clearing the site's enable.  The variable KEEPS its
    /// last assigned value (LRM 1364-1995 §9.4).  A `deassign` on a variable
    /// without any PCA site has no effect (warned); net/select/hierarchical
    /// targets are rejected like `assign` targets.  Sites are pre-allocated
    /// before any body lowers (`Codegen::prescan_pca_sites`), so this finds
    /// its site regardless of which process lowers first.
    fn lower_deassign(&mut self, lhs: NodeId) -> Result<Vec<IrStmt>, String> {
        let (sig_idx, _) = self.pca_target(lhs, "deassign")?;
        match self.cg.pca_sites.get(&sig_idx) {
            Some(site) => {
                let en_ir = site.en;
                let zero = IrExpr::new(
                    IrExprKind::Const(IrConst {
                        bits: vec![0],
                        x: vec![0],
                        z: vec![0],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: None,
                    }),
                    1,
                    false,
                    None,
                );
                Ok(vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(en_ir),
                    rhs: zero,
                    nba: false,
                }])
            }
            None => {
                self.cg.warnings.push(format!(
                    "deassign of `{}` in `{}` has no effect (no procedural \
                     continuous assignment on this variable)",
                    self.cg.node(lhs).name,
                    self.path
                ));
                Ok(vec![IrStmt::Nop])
            }
        }
    }

    /// Lower system-task calls ($display/$monitor/$strobe/$finish/…).
    /// Skippable constructs warn here and produce no statements.
    fn lower_sys_call(&mut self, h: NodeId, name: &str) -> Result<Vec<IrStmt>, String> {
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        match name {
            "$display" | "$write" => {
                let (fmt, display_args) = self.parse_display_call(name, &args, true)?;
                Ok(vec![IrStmt::Display {
                    fmt,
                    args: display_args,
                    newline: name == "$display",
                }])
            }
            "$monitor" | "$strobe" => {
                if let Some(local) = args
                    .iter()
                    .find_map(|arg| self.cg.nested_proc_local_ref(*arg))
                {
                    return Err(format!(
                        "{name} cannot defer a reference to inline loop variable `{}` in `{}`",
                        self.cg.node(local).name,
                        self.path
                    ));
                }
                if self.in_final {
                    return Err(format!(
                        "{name} inside a final block in `{}` is not supported: \
                         no scheduled output events execute after final procedures",
                        self.path
                    ));
                }
                let (fmt, display_args) = self.parse_display_call(name, &args, false)?;
                // Generated re-evaluator: reads the CURRENT values of the
                // displayed arguments each time the runtime prints (after an
                // NBA commit for the monitor, at the end of the time step for
                // $strobe).  Attached ahead of the enclosing function/process.
                let eval_name = self.cg.new_fn_name(&self.path, "mon");
                let eval_args = display_args.iter().map(|(e, _)| e.clone()).collect();
                self.pre_fns.push(crate::sim::ir::IrPreFn::MonEval {
                    c_name: eval_name.clone(),
                    args: eval_args,
                });
                Ok(vec![IrStmt::MonitorSet {
                    strobe: name == "$strobe",
                    fmt,
                    eval: eval_name,
                    n_args: display_args.len(),
                }])
            }
            "$monitoron" => Ok(vec![IrStmt::MonitorEnable(true)]),
            "$monitoroff" => Ok(vec![IrStmt::MonitorEnable(false)]),
            "$dumpfile" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumpfile requires exactly one literal string argument in `{}`",
                        self.path
                    ));
                }
                let path = match self.cg.kind(args[0]) {
                    NodeKind::Expr(ExprKind::Constant {
                        const_type: ConstantType::String,
                        value: ValueData::Str(path),
                        ..
                    }) => path.clone(),
                    _ => {
                        return Err(format!(
                            "$dumpfile requires a literal string argument in `{}`",
                            self.path
                        ))
                    }
                };
                let lower_path = path.to_ascii_lowercase();
                if !lower_path.ends_with(".vcd") && !lower_path.ends_with(".fst") {
                    return Err(format!(
                        "$dumpfile path must end in .vcd or .fst in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveFile(path)])
            }
            "$dumpvars" => {
                if !args.is_empty() && !self.cg.warned_dumpvars_filtering {
                    self.cg.warnings.push(
                        "$dumpvars depth/scope filtering is not yet implemented; dumping all \
                         registered storage"
                            .to_string(),
                    );
                    self.cg.warned_dumpvars_filtering = true;
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveDumpVars])
            }
            "$dumpon" | "$dumpoff" | "$dumpall" | "$dumpflush" => {
                if !args.is_empty() {
                    return Err(format!("{name} takes no arguments in `{}`", self.path));
                }
                self.cg.model.waveform = true;
                Ok(vec![match name {
                    "$dumpon" => IrStmt::WaveOn,
                    "$dumpoff" => IrStmt::WaveOff,
                    "$dumpall" => IrStmt::WaveDumpAll,
                    "$dumpflush" => IrStmt::WaveFlush,
                    _ => unreachable!(),
                }])
            }
            "$dumplimit" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumplimit requires exactly one packed expression in `{}`",
                        self.path
                    ));
                }
                let limit = self.cg.lower_expr(&self.path, args[0])?;
                if limit.is_real() {
                    return Err(format!(
                        "$dumplimit requires a packed expression, not real, in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveLimit(limit)])
            }
            "$finish" => Ok(vec![IrStmt::Finish]),
            "$printtimescale" => {
                let ts = self.cg.timescale_of_node(h);
                Ok(vec![IrStmt::PrintTimescale {
                    unit_ps: ts.unit_ps,
                    precision_ps: ts.precision_ps,
                    label: self.path.clone(),
                }])
            }
            "$displayon" | "$displayoff" => {
                self.cg.warnings.push(format!(
                    "{name} in `{}` skipped (not supported in v1)",
                    self.path
                ));
                Ok(vec![])
            }
            _ => Err(format!("unsupported system task {name} in `{}`", self.path)),
        }
    }

    /// Parse a $display/$monitor/$strobe call's arguments into the C format
    /// string and the lowered value expressions (with their realness flags).
    /// The first string constant is the format; every remaining argument is a
    /// value expression consumed by one format specifier (`%d/%h/%b/%o/%t`,
    /// `%s` only when `allow_strings`).  `allow_strings` is true for $display
    /// (whose `llg_display` reads string arguments from the varargs);
    /// monitors and strobes pass false because their arguments are
    /// re-evaluated as `sv4_t` values by the generated eval function.
    fn parse_display_call(
        &mut self,
        name: &str,
        args: &[NodeId],
        allow_strings: bool,
    ) -> Result<(String, Vec<(IrExpr, bool)>), String> {
        let mut fmt_arg: Option<String> = None;
        let mut display_args = Vec::new();
        for a in args {
            let is_fmt = matches!(
                self.cg.kind(*a),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    ..
                })
            );
            if is_fmt && fmt_arg.is_none() {
                fmt_arg = match self.cg.kind(*a) {
                    NodeKind::Expr(ExprKind::Constant {
                        value: ValueData::Str(s),
                        ..
                    }) => Some(s.clone()),
                    _ => None,
                };
            } else {
                let e = self.cg.lower_expr(&self.path, *a)?;
                display_args.push((e.clone(), e.is_real()));
            }
        }
        let fmt =
            fmt_arg.ok_or_else(|| format!("{name} without a format string in `{}`", self.path))?;
        if !allow_strings && display_args.iter().any(|(e, _)| e.is_real()) {
            return Err(format!(
                "{name} cannot monitor/strobe real-valued arguments in `{}`",
                self.path
            ));
        }
        let mut c_fmt = String::from("\"");
        let mut arg_idx = 0usize;
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                c_fmt.push_str(&escaped_char(ch));
                continue;
            }
            let mut spec = String::from("%");
            while let Some(&n) = chars.peek() {
                if n == '-' || n == '+' || n == '0' || n == '.' || n.is_ascii_digit() {
                    spec.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let conv = chars.next().unwrap_or('%');
            spec.push(conv);
            match conv {
                'd' | 'h' | 'b' | 'o' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if display_args[arg_idx].0.is_real() {
                        return Err(format!(
                            "{name} integer format `%{conv}` cannot consume a real in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push(conv);
                }
                's' => {
                    if !allow_strings {
                        return Err(format!(
                            "{name} format `%s` in `{}` is not supported (monitor/\
                             strobe arguments are re-evaluated as values)",
                            self.path
                        ));
                    }
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push(conv);
                }
                'f' | 'e' | 'g' => {
                    if !allow_strings {
                        return Err(format!(
                            "{name} real formatting is not supported for monitor/strobe in `{}`",
                            self.path
                        ));
                    }
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !display_args[arg_idx].0.is_real() {
                        return Err(format!(
                            "{name} real format `%{conv}` requires a real argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec);
                }
                't' => {
                    // %t consumes an argument (typically $time); the runtime
                    // prints the argument's value as a decimal (already scaled
                    // to the caller's time unit by the $time emission).
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push('t');
                }
                '%' => c_fmt.push('%'),
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{other}` in `{}`",
                        self.path
                    ))
                }
            }
        }
        if arg_idx != display_args.len() {
            return Err(format!(
                "{name} in `{}` has {} argument(s) for {} format specifier(s)",
                self.path,
                display_args.len(),
                arg_idx
            ));
        }
        c_fmt.push('"');
        Ok((c_fmt, display_args))
    }

    /// Lower a `task_call` statement (or a function call used as a statement).
    /// Delay-bearing tasks are inlined at the call site; delay-free tasks (and
    /// functions) become IR calls with caller-side temps for output formals.
    fn lower_task_call(
        &mut self,
        h: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<IrStmt, String> {
        if let Some(f) = &self.func {
            if !f.is_task {
                return Err(format!(
                    "task call `{name}` inside function `{}` is not supported",
                    f.name
                ));
            }
        }
        let ft = self.cg.resolve_callee(self.inst, name, is_task, callee)?;
        let automatic = matches!(
            self.cg.kind(ft),
            NodeKind::FuncTask {
                automatic: true,
                ..
            }
        );
        if is_task
            && self
                .cg
                .func_body(ft)
                .is_some_and(|body| self.cg.node_has_unsafe_subroutine_nba(body, ft, automatic))
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
        let (_, _, formals) = self.cg.func_info(ft, self.inst)?;
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        let bound = self.cg.bind_call_args(self.inst, &formals, &args)?;
        if is_task && self.cg.task_has_wait(ft, self.inst) {
            self.lower_task_inline(ft, h, &formals, &bound)
        } else {
            let fidx = self
                .cg
                .func_meta
                .get(&ft)
                .map(|m| m.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            self.lower_call_stmts(fidx, h, &formals, &bound)
        }
    }

    /// Lower a delay-free task/function statement call: caller-side temps for
    /// output/inout formals followed by copy-out, and inputs by value.
    fn lower_call_stmts(
        &mut self,
        fidx: usize,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<IrStmt, String> {
        let mut temps: Vec<(String, usize, Option<IrExpr>)> = Vec::new();
        let mut copyouts: Vec<(IrLhs, String, u32, bool)> = Vec::new();
        // The C signature orders all outputs first, then inputs — build the
        // argument list in that order, not by formal declaration index.
        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            let lh = self.cg.lower_lhs(&self.path, bound[idx].expr)?;
            if let Some(storage) = self.cg.static_formals.get(&(self.inst, *io)).cloned() {
                let storage_lhs = IrLhs::Whole(storage.ir);
                if matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                ) {
                    let value = self.cg.lower_expr(&self.path, bound[idx].expr)?;
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
                after.push(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.cg.model, &lh, read.clone()),
                    lhs: lh,
                    nba: false,
                });
                arg_irs[idx] = Some(read);
                arg_codes[idx] = Some(storage.global.clone());
                out_args.push(IrCallArg::OutAddr(format!("&{}", storage.global)));
                continue;
            }
            let tname = format!("_a{}_{}", h.0, idx);
            let (_init_code, init_ir) =
                self.cg.lower_call_temp_init(&self.path, *io, &bound[idx])?;
            temps.push((tname.clone(), idx, init_ir));
            copyouts.push((lh, tname.clone(), bound[idx].width, bound[idx].signed));
            arg_irs[idx] = Some(IrExpr::new(
                IrExprKind::LocalRead(tname.clone()),
                bound[idx].width,
                bound[idx].signed,
                None,
            ));
            arg_codes[idx] = Some(tname.clone());
            out_args.push(IrCallArg::OutAddr(format!("&{tname}")));
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                let mut own_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
                let (_code, ir) = self.cg.lower_bound_arg_code(
                    &self.path,
                    formals,
                    bound,
                    idx,
                    &mut arg_codes,
                    &mut own_irs,
                )?;
                in_args.push(IrCallArg::Val(ir));
            }
        }
        out_args.extend(in_args);
        let depth = parse_depth(&self.depth_arg);
        let call = IrStmt::Call(IrCall {
            f: fidx,
            args: out_args,
            depth,
            temps,
            copyouts,
        });
        if before.is_empty() && after.is_empty() {
            Ok(call)
        } else {
            before.push(call);
            before.extend(after);
            Ok(IrStmt::Block(before))
        }
    }

    /// Lower a delay-bearing task body inlined at its call site: the task's
    /// io_decls are bound to the caller's argument expressions (writes go
    /// straight to the bound actuals through the func-context remap), locals
    /// get fresh names, and the body lowers under the inline context.  A call
    /// to a task already being inlined (recursion) is rejected.
    fn lower_task_inline(
        &mut self,
        ft: NodeId,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<IrStmt, String> {
        let tname = self.cg.node(ft).name.clone();
        if let Some(inl) = &self.inline {
            if inl.chain.contains(&tname) {
                return Err(format!(
                    "recursive delay-bearing task `{tname}` is not supported"
                ));
            }
        }
        let body = self
            .cg
            .func_body(ft)
            .ok_or_else(|| format!("task `{tname}` without a body"))?;

        // Task locals get fresh C names per inline site (the same task may be
        // inlined several times in one block).
        let mut locals: HashMap<NodeId, (String, u32, bool, bool)> = HashMap::new();
        let mut local_seq = 0usize;
        let prefix = format!("_i{}", h.0);
        self.cg
            .collect_func_locals(body, self.inst, &mut locals, &mut local_seq, &prefix)?;

        // Formals bound to the caller's argument expressions.
        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        // Input formals bound to caller rvalue expressions need a writable
        // local copy (an input formal is a local copy in SystemVerilog).
        let mut input_copies: Vec<(String, IrExpr, bool)> = Vec::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let b = &bound[idx];
            if *is_out {
                let lh = self.cg.lower_lhs(&self.path, b.expr)?;
                let addr = match &lh {
                    IrLhs::Whole(sig_i) => format!("&{}", self.cg.model.signal(*sig_i).c_name),
                    IrLhs::WholeRef { addr, .. } => addr.clone(),
                    _ => {
                        return Err(format!(
                            "output argument of task `{tname}` bound to a select \
                             is not supported"
                        ))
                    }
                };
                let read_ir = self.cg.lower_expr(&self.path, b.expr)?;
                arg_write.insert(*io, addr);
                arg_ir.insert(*io, read_ir.clone());
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: b.width,
                        signed: b.signed,
                        two_state: b.two_state,
                    },
                );
                arg_irs[idx] = Some(read_ir.clone());
                arg_codes[idx] = Some(self.cg.render_ir_code(&read_ir)?);
            } else {
                let (_code, ir) = self.cg.lower_bound_arg_code(
                    &self.path,
                    formals,
                    bound,
                    idx,
                    &mut arg_codes,
                    &mut arg_irs,
                )?;
                let cname = format!("_il{}_{}", h.0, idx);
                arg_write.insert(*io, format!("&{cname}"));
                arg_ir.insert(
                    *io,
                    IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        b.width,
                        b.signed,
                        None,
                    ),
                );
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: b.width,
                        signed: b.signed,
                        two_state: b.two_state,
                    },
                );
                input_copies.push((cname, ir, b.two_state));
            }
        }

        let done_label = format!("_id{}", h.0);
        let mut chain = match &self.inline {
            Some(inl) => inl.chain.clone(),
            None => Vec::new(),
        };
        chain.push(tname.clone());
        let func = FuncCtx {
            name: tname.clone(),
            is_task: true,
            ret: None,
            arg_read,
            arg_ir,
            arg_write,
            locals,
            ret_node: None,
            // Deliberately None: a `disable <taskname>;` inside an inlined
            // body must jump to THIS expansion's done label (the InlineCtx
            // check runs first), never emit a C return out of the caller's
            // coroutine.
            def_node: None,
        };
        let inline = InlineCtx {
            done_label: done_label.clone(),
            def: ft,
            chain,
            used: false,
        };

        // Swap in the inline context (and sync the codegen for expression
        // resolution), then restore on the way out.
        let saved_cg = (self.cg.func.take(), self.cg.depth_arg.clone());
        let saved_ctx = (self.func.take(), self.inline.take(), self.depth_arg.clone());
        let depth = format!("({}) + 1", saved_ctx.2);
        self.cg.func = Some(func.clone());
        self.cg.depth_arg = depth.clone();
        self.func = Some(func);
        self.inline = Some(inline);
        self.depth_arg = depth;

        let mut stmts: Vec<IrStmt> = Vec::new();
        // Locals sorted by node id (emission order of the pre-IR emitter).
        let mut local_names: Vec<(u32, (String, u32, bool, bool))> = self
            .func
            .as_ref()
            .map(|f| {
                f.locals
                    .iter()
                    .map(|(id, v)| (id.0, v.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        local_names.sort_by_key(|(id, _)| *id);
        for (_, (cname, w, s, two_state)) in local_names {
            stmts.push(IrStmt::DeclLocal {
                name: cname,
                width: w,
                signed: s,
                two_state,
                init: None,
            });
        }
        for (cname, ir, two_state) in input_copies {
            let (width, signed) = (ir.width, ir.signed);
            stmts.push(IrStmt::DeclLocal {
                name: cname,
                width,
                signed,
                two_state,
                init: Some(Box::new(ir)),
            });
        }
        // The body expands within the caller's control stack; the TaskBody
        // barrier keeps a `break`/`continue` inside it from binding to one
        // of the caller's loops (it must resolve within the task body or be
        // rejected cleanly).
        self.ctrl.push(CtrlScope::TaskBody);
        stmts.extend(self.lower_stmt(body)?);
        match self.ctrl.pop() {
            Some(CtrlScope::TaskBody) => {}
            _ => unreachable!("task-body scope stack imbalance"),
        }
        if self.inline.as_ref().map(|i| i.used).unwrap_or(false) {
            stmts.push(IrStmt::Label(done_label));
        }

        self.cg.func = saved_cg.0;
        self.cg.depth_arg = saved_cg.1;
        self.func = saved_ctx.0;
        self.inline = saved_ctx.1;
        self.depth_arg = saved_ctx.2;
        Ok(IrStmt::Block(stmts))
    }

    /// Lower a `return` statement.  In a non-void function the value becomes
    /// the `_ret` conversion + C return (rendered); in a void function/task it
    /// is a bare return.  Inside an inlined task body it jumps to the done
    /// label.
    fn lower_return(&mut self, value: Option<NodeId>) -> Result<IrStmt, String> {
        if let Some(inl) = self.inline.as_mut() {
            if value.is_some() {
                return Err("return with a value inside a task".to_string());
            }
            inl.used = true;
            return Ok(IrStmt::Goto(inl.done_label.clone()));
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

/// Wildcard comparisons are context-determined: widening must reach into a
/// nested arithmetic/bitwise operand before it is evaluated (for example, a
/// four-bit addition compared with a five-bit pattern retains its carry).
fn wildcard_case_match(
    scope_path: &str,
    selector: IrExpr,
    value: IrExpr,
) -> Result<IrExpr, String> {
    let width = selector.width.max(value.width);
    let signed = selector.signed && value.signed;
    let selector = wildcard_operand_with_context(selector, width, signed, scope_path)?;
    let value = wildcard_operand_with_context(value, width, signed, scope_path)?;
    Ok(cmp_expr_ir(IrBinOp::WildEq, selector, value))
}

fn case_inside_range_match(
    scope_path: &str,
    selector: IrExpr,
    low: IrExpr,
    high: IrExpr,
) -> Result<IrExpr, String> {
    let low_width = selector.width.max(low.width);
    let low_signed = selector.signed && low.signed;
    let low_selector =
        wildcard_operand_with_context(selector.clone(), low_width, low_signed, scope_path)?;
    let low = wildcard_operand_with_context(low, low_width, low_signed, scope_path)?;
    let ge = cmp_expr_ir(IrBinOp::Ge, low_selector, low);

    let high_width = selector.width.max(high.width);
    let high_signed = selector.signed && high.signed;
    let high_selector =
        wildcard_operand_with_context(selector, high_width, high_signed, scope_path)?;
    let high = wildcard_operand_with_context(high, high_width, high_signed, scope_path)?;
    let le = cmp_expr_ir(IrBinOp::Le, high_selector, high);
    Ok(cmp_expr_ir(IrBinOp::LogAnd, ge, le))
}
