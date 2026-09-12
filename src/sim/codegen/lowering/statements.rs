//! Procedural statement lowering through the shared emission context.

use super::*;
use crate::sim::ir::IrStringExpr;
use crate::sim::ir::IrObjectStmt;

#[derive(Clone, Copy)]
enum DisplayTaskKind {
    Immediate { newline: bool },
    Deferred { strobe: bool },
}

fn display_task_variant(name: &str) -> Option<(DisplayTaskKind, IrDisplayRadix)> {
    let variant = match name {
        "$display" => (
            DisplayTaskKind::Immediate { newline: true },
            IrDisplayRadix::Decimal,
        ),
        "$displayb" => (
            DisplayTaskKind::Immediate { newline: true },
            IrDisplayRadix::Binary,
        ),
        "$displayo" => (
            DisplayTaskKind::Immediate { newline: true },
            IrDisplayRadix::Octal,
        ),
        "$displayh" => (
            DisplayTaskKind::Immediate { newline: true },
            IrDisplayRadix::Hex,
        ),
        "$write" => (
            DisplayTaskKind::Immediate { newline: false },
            IrDisplayRadix::Decimal,
        ),
        "$writeb" => (
            DisplayTaskKind::Immediate { newline: false },
            IrDisplayRadix::Binary,
        ),
        "$writeo" => (
            DisplayTaskKind::Immediate { newline: false },
            IrDisplayRadix::Octal,
        ),
        "$writeh" => (
            DisplayTaskKind::Immediate { newline: false },
            IrDisplayRadix::Hex,
        ),
        "$strobe" => (
            DisplayTaskKind::Deferred { strobe: true },
            IrDisplayRadix::Decimal,
        ),
        "$strobeb" => (
            DisplayTaskKind::Deferred { strobe: true },
            IrDisplayRadix::Binary,
        ),
        "$strobeo" => (
            DisplayTaskKind::Deferred { strobe: true },
            IrDisplayRadix::Octal,
        ),
        "$strobeh" => (
            DisplayTaskKind::Deferred { strobe: true },
            IrDisplayRadix::Hex,
        ),
        "$monitor" => (
            DisplayTaskKind::Deferred { strobe: false },
            IrDisplayRadix::Decimal,
        ),
        "$monitorb" => (
            DisplayTaskKind::Deferred { strobe: false },
            IrDisplayRadix::Binary,
        ),
        "$monitoro" => (
            DisplayTaskKind::Deferred { strobe: false },
            IrDisplayRadix::Octal,
        ),
        "$monitorh" => (
            DisplayTaskKind::Deferred { strobe: false },
            IrDisplayRadix::Hex,
        ),
        _ => return None,
    };
    Some(variant)
}

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
            process_kind: None,
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

    /// Ordinary functions cannot suspend, but a task body emitted as a typed
    /// C call runs inside the caller's libaco coroutine and may yield. Inline
    /// task expansion is still allowed for the event/cancellation paths that
    /// need caller-owned activation rebinding.
    fn timing_forbidden(&self) -> bool {
        self.inline.is_none()
            && self
                .func
                .as_ref()
                .is_some_and(|function| !function.is_task)
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
        let cond = self.cg.lower_boolean_expr(&self.path, cond_node)?;
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
    /// outside a loop this is a syntax-level error that the frontend normally
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
                CtrlScope::Block => {}
            }
        }
        Err(format!(
            "`{}` outside a loop in `{}`",
            if is_break { "break" } else { "continue" },
            self.path
        ))
    }

    /// Disable all active invocations of the resolved block or task, including
    /// self-disable. This is not an early return from only the current call.
    fn lower_disable(&mut self, target: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        let target = target.ok_or_else(|| {
            format!("cannot resolve the target of `disable` in `{}`", self.path)
        })?;
        Ok(vec![IrStmt::DisableTarget {
            target: self.cg.activation_target(target)?,
        }])
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
                let children = self.cg.node(h).children.clone();
                if self.func.is_none() {
                    for child in &children {
                        if matches!(self.cg.kind(*child), NodeKind::Var { .. }) {
                            let info = self.cg.collect_loop_var(&self.path, *child)?;
                            if info.static_signal.is_none() {
                                body.push(IrStmt::DeclLocal {
                                    name: info.c_name,
                                    width: info.width,
                                    signed: info.signed,
                                    two_state: info.two_state,
                                    init: None,
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
                let activation_target = named
                    .then(|| self.cg.activation_target(h))
                    .transpose()?;
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
                    (Some(target), Some(exit)) => IrStmt::ActivationScope {
                        target,
                        exit,
                        body,
                    },
                    (None, None) => IrStmt::Block(body),
                    _ => unreachable!("named activation scope metadata mismatch"),
                };
                Ok(vec![statement])
            }
            NodeKind::Stmt(StmtKind::IfElse { cond }) => {
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
                Some(IntraControl::EventOrRepeat) => Err(format!(
                    "intra-assignment event/repeat control (`@(…)` or \
                     `repeat (n) @(…)`) in `{}` is not supported",
                    self.path
                )),
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
            NodeKind::MethodCall { .. } => {
                if let Some(statement) = self.cg.lower_container_method(&self.path, h)? {
                    Ok(vec![statement])
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

    fn lower_variable_decl(&mut self, declaration: NodeId) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() {
            return match self.cg.db.variable_lifetime(declaration) {
                VariableLifetime::Static => Ok(Vec::new()),
                VariableLifetime::Automatic => {
                    if matches!(self.cg.kind(declaration), NodeKind::Var { ty } if ty.kind == "chandle") {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.chandle_read.get(&declaration))
                            .and_then(|value| match value {
                                IrChandleExpr::LocalRead(name) => Some(name.clone()),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!(
                                    "automatic chandle variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| self.cg.lower_chandle(&self.path, initializer))
                            .transpose()?;
                        return Ok(vec![IrStmt::Object(
                            IrObjectStmt::ChandleDeclareLocal(name, init),
                        )]);
                    }
                    if matches!(self.cg.kind(declaration), NodeKind::Var { ty } if ty.kind == "string") {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.locals.get(&declaration))
                            .map(|(name, ..)| name.clone())
                            .ok_or_else(|| {
                                format!(
                                    "automatic string variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| self.cg.lower_string(&self.path, initializer))
                            .transpose()?;
                        return Ok(vec![IrStmt::DeclString { name, init }]);
                    }
                    let (_, width, signed, two_state, shortreal) = self
                        .func
                        .as_ref()
                        .and_then(|function| function.locals.get(&declaration))
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "automatic subprogram variable `{}` has no local storage",
                                self.cg.node(declaration).name
                            )
                        })?;
                    let init = self
                        .cg
                        .db
                        .var_initializer(declaration)
                        .map(|initializer| {
                            let expression = self.cg.lower_expr(&self.path, initializer)?;
                            if width == 0 {
                                Ok(IrExpr::new(
                                    IrExprKind::CastToReal {
                                        a: Box::new(expression),
                                        shortreal,
                                    },
                                    0,
                                    false,
                                    None,
                                ))
                            } else {
                                ir_to_storage(expression, width, signed, two_state)
                            }
                            .map(Box::new)
                        })
                        .transpose()?;
                    let name = self
                        .func
                        .as_ref()
                        .and_then(|function| function.locals.get(&declaration))
                        .map(|(name, ..)| name.clone())
                        .expect("automatic local storage checked above");
                    Ok(vec![IrStmt::DeclLocal {
                        name,
                        width,
                        signed,
                        two_state,
                        init,
                    }])
                }
                VariableLifetime::Unavailable => Err(format!(
                    "resolved lifetime is unavailable for subprogram variable `{}` in `{}`",
                    self.cg.node(declaration).name,
                    self.path
                )),
            };
        }

        let info = self.cg.collect_loop_var(&self.path, declaration)?;
        if info.static_signal.is_some() {
            return Ok(Vec::new());
        }
        let init = self
            .cg
            .db
            .var_initializer(declaration)
            .map(|initializer| {
                let expr = self.cg.lower_expr(&self.path, initializer)?;
                ir_to_storage(expr, info.width, info.signed, info.two_state).map(Box::new)
            })
            .transpose()?;
        Ok(vec![IrStmt::DeclLocal {
            name: info.c_name,
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            init,
        }])
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
        self.lower_assignment_operands(lhs, rhs, force_blocking || blocking, op, force_blocking)
    }

    fn lower_assignment_operands(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
        force_blocking: bool,
    ) -> Result<IrStmt, String> {
        // A subprogram declaration initializer can be represented as an
        // assignment whose LHS is the variable declaration itself. Its value is
        // emitted through `IrLocal::initial`; suppress only that structural
        // declaration form. Executable assignments have Ref LHS nodes.
        let is_declaration_initializer = matches!(self.cg.kind(lhs), NodeKind::Var { .. })
            && !force_blocking
            && self
                .func
                .as_ref()
                .is_some_and(|function| function.locals.contains_key(&lhs));
        if is_declaration_initializer {
            return Ok(IrStmt::Nop);
        }
        if let Some(target) = self.cg.event_target_of(lhs) {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to named event `{}` in `{}` is not supported",
                    self.cg.node(target.declaration).name,
                    self.path
                ));
            }
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to named event `{}` in `{}` is not supported",
                    self.cg.node(target.declaration).name,
                    self.path
                ));
            }
            let target = self.cg.event_ref_of(&target, &self.path)?;
            let source = if let Some(source) = self.cg.event_target_of(rhs) {
                Some(self.cg.event_ref_of(&source, &self.path)?)
            } else if matches!(
                self.cg.kind(rhs),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::Null,
                    ..
                })
            ) {
                None
            } else {
                return Err(format!(
                    "named event assignment in `{}` requires another event handle or null",
                    self.path
                ));
            };
            return Ok(IrStmt::EventAssign { target, source });
        }
        if let Some(statement) = self
            .cg
            .lower_container_assignment(&self.path, lhs, rhs, blocking, op)?
        {
            return Ok(statement);
        }
        if let Some(statement) = self
            .cg
            .lower_object_assignment(&self.path, lhs, rhs, blocking, op)?
        {
            return Ok(statement);
        }
        if !blocking {
            if let Some(local) = self.cg.proc_local_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to inline loop variable `{}` in `{}` is not supported because the update can outlive its lexical storage",
                    self.cg.node(local).name,
                    self.path
                ));
            }
            if self.cg.subroutine_auto_target(lhs) {
                return Err(format!(
                    "nonblocking assignment to automatic subroutine storage in `{}` is not supported because the update can outlive its activation",
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
        if let Some(aggregate_assignment) = self
            .cg
            .lower_unpacked_aggregate_assignment(&self.path, lhs, rhs, !blocking, op)?
        {
            return Ok(aggregate_assignment);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = match self
            .cg
            .lower_packed_aggregate_pattern(&self.path, lhs, rhs, op)?
        {
            Some(value) => value,
            None => self.lower_assignment_rhs(lhs, rhs, op, &lh)?,
        };
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
        op: Operation,
        lhs: &IrLhs,
    ) -> Result<IrExpr, String> {
        let rhs = self.cg.lower_expr(&self.path, rhs_node)?;
        if op == Operation::Assignment {
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

    fn lower_compound_expr(
        &self,
        op: Operation,
        lhs: IrExpr,
        rhs: IrExpr,
    ) -> Result<IrExpr, String> {
        super::lower_compound_expr_ir(&self.path, op, lhs, rhs)
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

    /// Capture a delayed assignment's RHS immediately. Blocking assignments
    /// suspend; nonblocking assignments enqueue a future NBA and continue.
    fn lower_delayed_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        scaled_ticks: IrDelay,
    ) -> Result<Vec<IrStmt>, String> {
        if self.timing_forbidden() {
            return Err(format!(
                "delay inside a function body in `{}` is not supported \
                 (ordinary functions cannot suspend)",
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
        if self.cg.is_string_expr(&self.path, lhs) {
            self
                .cg
                .ensure_string_actual_writable(&self.path, lhs)?;
            let target = self
                .cg
                .lower_string_actual_address(&self.path, lhs)?
                .trim_start_matches('&')
                .to_owned();
            let value = self.cg.lower_string(&self.path, rhs)?;
            if !blocking {
                if self.cg.proc_local_target(lhs).is_some() {
                    return Err(
                        "nonblocking delayed assignment requires persistent string storage"
                            .to_owned(),
                    );
                }
                return Ok(vec![IrStmt::DelayedStringAssign {
                    target,
                    rhs: value,
                    ticks: scaled_ticks,
                }]);
            }
            self.saw_wait = true;
            let tmp = format!("_st{}", h.0);
            return Ok(vec![IrStmt::Block(vec![
                IrStmt::DeclString {
                    name: tmp.clone(),
                    init: Some(value),
                },
                IrStmt::Delay {
                    ticks: scaled_ticks,
                },
                IrStmt::Object(IrObjectStmt::StringAssignLocal(
                    target,
                    IrStringExpr::LocalRead(tmp.clone()),
                )),
                IrStmt::Object(IrObjectStmt::StringAssignLocal(
                    tmp,
                    IrStringExpr::Literal(Vec::new()),
                )),
            ])]);
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_delayed_assignment"),
        };
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        if !blocking {
            if self.cg.proc_local_target(lhs).is_some() || matches!(lh, IrLhs::WholeRef { .. }) {
                return Err(
                    "nonblocking delayed assignment requires persistent target storage".into(),
                );
            }
            return Ok(vec![IrStmt::DelayedAssign {
                lhs: lh,
                rhs: rhs_ir,
                ticks: scaled_ticks,
            }]);
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
            let reads = if implicit && self.process_kind == Some(AlwaysKind::Always) {
                self.cg.collect_at_star_signals(&self.path, body)?
            } else {
                self.cg.collect_read_signals(&self.path, body)?
            };
            if implicit
                && self.process_kind == Some(AlwaysKind::Always)
                && reads.is_empty()
            {
                // An empty `@*` sensitivity list waits forever. Keep the
                // explicit never-triggered event as the source-level sentinel.
                let event = self.cg.model.events.len();
                self.cg.model.events.push(crate::sim::ir::IrEvent::new(
                    format!("E_{}_at_star_empty_{event}", ident(&self.path)),
                ));
                IrStmt::WaitEvents {
                    specs: vec![(IrWaitSrc::Event(IrEventRef::Static(event)), IrEdge::Any)],
                }
            } else {
                IrStmt::WaitAny { sens: reads }
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

    fn wait_body(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let mut body = Vec::new();
        if let Some(b) = self.cg.node(h).children.get(1) {
            if !matches!(self.cg.kind(*b), NodeKind::Stmt(StmtKind::Empty)) {
                body = self.lower_stmt(*b)?;
            }
        }
        Ok(body)
    }

    fn triggered_event_ref(&mut self, mut node: NodeId) -> Result<Option<IrEventRef>, String> {
        loop {
            let operand = match self.cg.kind(node) {
                NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
                _ => None,
            };
            if let Some(operand) = operand {
                node = operand;
                continue;
            }
            let receiver = match self.cg.kind(node) {
                NodeKind::MethodCall {
                    name,
                    receiver: Some(receiver),
                } if name == "triggered" => Some(*receiver),
                _ => None,
            };
            let Some(receiver) = receiver else {
                return Ok(None);
            };
            let target = self.cg.event_target_of(receiver).ok_or_else(|| {
                format!("event triggered property has an unresolved receiver in `{}`", self.path)
            })?;
            return self.cg.event_ref_of(&target, &self.path).map(Some);
        }
    }

    fn unsupported_event_trigger_timing(
        &self,
        timing: &EventTriggerTiming,
        reason: &str,
    ) -> String {
        let control = match timing {
            EventTriggerTiming::Delay { control, .. }
            | EventTriggerTiming::Event { control, .. }
            | EventTriggerTiming::Repeat { control, .. }
            | EventTriggerTiming::Unsupported { control } => *control,
        };
        let node = self.cg.node(control);
        let file = node.file.as_deref().unwrap_or("<unknown>");
        format!(
            "{reason} at {file}:{}:{} in `{}`",
            node.line, node.col, self.path
        )
    }

    fn lower_nonblocking_event_trigger(
        &mut self,
        statement: NodeId,
        event: IrEventRef,
        timing: Option<&EventTriggerTiming>,
    ) -> Result<Vec<IrStmt>, String> {
        let Some(timing) = timing else {
            return Ok(vec![IrStmt::NonblockingEventTrigger {
                ev: event,
                ticks: None,
            }]);
        };
        match timing {
            EventTriggerTiming::Delay { expression, .. } => {
                let ticks = self
                    .cg
                    .lower_procedural_delay(&self.path, statement, *expression)?;
                Ok(vec![IrStmt::NonblockingEventTrigger {
                    ev: event,
                    ticks: Some(ticks),
                }])
            }
            EventTriggerTiming::Event { specs, .. } => Ok(vec![
                IrStmt::NonblockingEventTriggerWhen {
                    ev: event,
                    specs: self.lower_event_specs(specs)?,
                    repeat: None,
                },
            ]),
            EventTriggerTiming::Repeat { count, event: inner, .. } => {
                let EventTriggerTiming::Event { specs, .. } = inner.as_ref() else {
                    return Err(self.unsupported_event_trigger_timing(
                        timing,
                        "repeat nonblocking event triggers require an event control",
                    ));
                };
                let repeat = self.cg.lower_expr(&self.path, *count)?;
                if repeat.is_real() {
                    return Err(self.unsupported_event_trigger_timing(
                        timing,
                        "repeat nonblocking event triggers require a packed count",
                    ));
                }
                Ok(vec![IrStmt::NonblockingEventTriggerWhen {
                    ev: event,
                    specs: self.lower_event_specs(specs)?,
                    repeat: Some(repeat),
                }])
            }
            EventTriggerTiming::Unsupported { .. } => Err(self.unsupported_event_trigger_timing(
                timing,
                "unsupported nonblocking event-trigger timing",
            )),
        }
    }

    /// Resolve the captured event specs into atomic wait sources: signal
    /// globals with edges, plus named events.  A mixed list stays ONE
    /// `WaitEvents` statement (one runtime call), so a trigger can never be
    /// lost between two separate waits.
    fn lower_event_specs(
        &mut self,
        specs: &[EventSpec],
    ) -> Result<Vec<(IrWaitSrc, IrEdge)>, String> {
        specs
            .iter()
            .map(|spec| self.lower_event_spec(spec, None))
            .collect()
    }

    fn event_evaluator(&mut self, expression: NodeId) -> Result<(String, bool), String> {
        self.cg
            .check_event_expression_effects(expression, &self.path)?;
        let value = self.cg.lower_expr(&self.path, expression)?;
        let context = self.cg.event_context(expression)?;
        let name = self.cg.new_fn_name(&self.path, "event_eval");
        let real = value.is_real();
        if real {
            self.pre_fns.push(crate::sim::ir::IrPreFn::RealEval {
                c_name: name.clone(),
                value,
                context,
            });
        } else {
            self.pre_fns.push(crate::sim::ir::IrPreFn::MonEval {
                c_name: name.clone(),
                args: vec![value],
                context,
            });
        }
        Ok((name, real))
    }

    fn lower_event_spec(
        &mut self,
        spec: &EventSpec,
        condition: Option<NodeId>,
    ) -> Result<(IrWaitSrc, IrEdge), String> {
        let (expression, edge, event) = match spec {
            EventSpec::Qualified { event, condition } => {
                return self.lower_event_spec(event, Some(*condition))
            }
            EventSpec::Named(event) => {
                let target = self.cg.event_target_of(*event).ok_or_else(|| {
                    format!("event control has an unresolved named event in `{}`", self.path)
                })?;
                (None, IrEdge::Any, Some(target))
            }
            EventSpec::AnyChange { sig } => {
                (Some(*sig), IrEdge::Any, self.cg.event_target_of(*sig))
            }
            EventSpec::Edge { sig, posedge } => {
                if self.cg.event_target_of(*sig).is_some() {
                    return Err(format!(
                        "edge control on a named event is not supported in `{}`",
                        self.path
                    ));
                }
                (
                    Some(*sig),
                    if *posedge {
                        IrEdge::Posedge
                    } else {
                        IrEdge::Negedge
                    },
                    None,
                )
            }
        };
        let condition = condition
            .map(|expression| {
                self.event_evaluator(expression).and_then(|(name, real)| {
                    if real {
                        Err(format!(
                            "real-valued event qualifiers are not supported in `{}`",
                            self.path
                        ))
                    } else {
                        Ok(name)
                    }
                })
        })
        .transpose()?;
        if let Some(event) = event {
            let event = self.cg.event_ref_of(&event, &self.path)?;
            return Ok((
                match condition {
                    Some(condition) => IrWaitSrc::FilteredEvent { event, condition },
                    None => IrWaitSrc::Event(event),
                },
                edge,
            ));
        }
        let expression = expression.ok_or_else(|| "event control has no expression".to_string())?;
        let simple = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        );
        let mapped_formal = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self
                .func
                .as_ref()
                .is_some_and(|func| func.arg_ir.contains_key(target))
        );
        if simple
            && condition.is_none()
            && self.cg.nested_proc_local_ref(expression).is_none()
            && !mapped_formal
        {
            let (name, info) = self.cg.resolve_signal_id(&self.path, expression)?;
            if info.real {
                if edge != IrEdge::Any {
                    return Err(format!(
                        "edge control on real-valued signal `{name}` is not supported in `{}`",
                        self.path
                    ));
                }
                return Ok((IrWaitSrc::Real(name), edge));
            }
            return Ok((IrWaitSrc::Sig(name), edge));
        }
        let reads = self.cg.collect_read_signals(&self.path, expression)?;
        let (eval, real) = self.event_evaluator(expression)?;
        if real && edge != IrEdge::Any {
            return Err(format!(
                "edge control on real-valued expressions is not supported in `{}`",
                self.path
            ));
        }
        Ok((
            if real {
                IrWaitSrc::EvaluatedReal {
                    eval,
                    condition,
                    reads,
                }
            } else {
                IrWaitSrc::Evaluated {
                    eval,
                    condition,
                    reads,
                }
            },
            edge,
        ))
    }

    fn lower_case(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (case_type, items) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Case { case_type, items }) => (*case_type, items),
            _ => unreachable!("non-case passed to lower_case"),
        };
        let sel = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "case without selector".to_string())?;
        let mut sel_ir = self.cg.lower_expr(&self.path, sel)?;
        if case_type == DbCaseKind::Inside {
            return self.lower_case_inside(items, sel_ir);
        }
        // `casez` and `casex` keep wildcard matching per LRM 12.5.1 instead
        // of degrading to exact equality.
        let kind = match case_type {
            DbCaseKind::Exact => IrCaseKind::Exact,
            DbCaseKind::X => IrCaseKind::Casex,
            DbCaseKind::Z => IrCaseKind::Casez,
            other => {
                return Err(format!(
                    "unsupported case type {other:?} in `{}`",
                    self.path
                ))
            }
        };
        let mut ir_items = Vec::with_capacity(items.len());
        let mut common_width = sel_ir.width;
        let mut common_signed = sel_ir.signed;
        let mut has_real = sel_ir.is_real();
        for item in items.iter() {
            let mut exprs = Vec::with_capacity(item.exprs.len());
            for e in &item.exprs {
                let c = self.cg.lower_expr(&self.path, *e)?;
                has_real |= c.is_real();
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
        if has_real {
            if case_type != DbCaseKind::Exact {
                return Err(format!(
                    "real-valued case forms require ordinary `case` in `{}`",
                    self.path
                ));
            }
            // A case selector is evaluated once, including a side-effecting
            // function call and even a case containing only a default item.
            let selector_name = self.new_label("real_case");
            let selector_init = IrStmt::DeclLocal {
                name: selector_name.clone(),
                width: sel_ir.width,
                signed: sel_ir.signed,
                two_state: false,
                init: Some(Box::new(sel_ir.clone())),
            };
            sel_ir = IrExpr::new(
                IrExprKind::LocalRead(selector_name),
                sel_ir.width,
                sel_ir.signed,
                None,
            );
            let mut default = None;
            let mut branches = Vec::new();
            for item in ir_items {
                if item.exprs.is_empty() {
                    if default.replace(item.body).is_some() {
                        return Err(format!(
                            "case has multiple default items in `{}`",
                            self.path
                        ));
                    }
                    continue;
                }
                let mut condition = None;
                for expression in item.exprs {
                    let matched =
                        common_cmp_expr_ir(IrBinOp::Eq, sel_ir.clone(), expression, &self.path)?;
                    condition = Some(match condition {
                        Some(previous) => cmp_expr_ir(IrBinOp::LogOr, previous, matched),
                        None => matched,
                    });
                }
                branches.push((condition.expect("non-empty case item"), item.body));
            }
            let mut tail = default.unwrap_or_default();
            for (condition, body) in branches.into_iter().rev() {
                tail = vec![IrStmt::If {
                    cond: condition,
                    then_: body,
                    els: (!tail.is_empty()).then_some(tail),
                }];
            }
            tail.insert(0, selector_init);
            return Ok(tail);
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

            let mut condition = None;
            for operand in &item.exprs {
                let members = match self.cg.kind(*operand) {
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::List,
                        operands,
                        ..
                    }) => operands.clone(),
                    _ => vec![*operand],
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
                    _ => {
                        return Err(format!(
                            "case-inside range requires two endpoints in `{}`",
                            self.path
                        ));
                    }
                };
                condition = Some(match condition {
                    Some(previous) => cmp_expr_ir(IrBinOp::LogOr, previous, matched),
                    None => matched,
                });
            }
            branches.push((
                condition.ok_or_else(|| "case-inside item has no expressions".to_string())?,
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
                NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => {
                    let op = *op;
                    validate_operation_arity(op, operands.len(), &self.path)?;
                    let [lhs, rhs] = operands.as_slice() else {
                        return Err(format!(
                            "for-loop assignment initializer in `{}` requires two operands",
                            self.path
                        ));
                    };
                    let (lhs, rhs) = (*lhs, *rhs);
                    init_stmts.push(self.lower_assignment_operands(lhs, rhs, true, op, true)?);
                }
                other => {
                    return Err(format!(
                        "unsupported for-loop initializer in `{}` (node kind {other:?})",
                        self.path
                    ))
                }
            }
        }
        let cond_ir = self.cg.lower_boolean_expr(&self.path, cond)?;
        let (body_stmts, brk) = self.lower_loop_body(body)?;
        let mut incr_stmts = Vec::with_capacity(incr.len());
        for s in &incr {
            match self.cg.kind(*s) {
                NodeKind::Stmt(StmtKind::Assign { .. }) => {
                    incr_stmts.push(self.lower_assignment(*s, true)?);
                }
                NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                    if !matches!(
                        *op,
                        Operation::PostIncrement
                            | Operation::PreIncrement
                            | Operation::PostDecrement
                            | Operation::PreDecrement
                    ) =>
                {
                    let op = *op;
                    validate_operation_arity(op, operands.len(), &self.path)?;
                    let [lhs, rhs] = operands.as_slice() else {
                        return Err(format!(
                            "for-loop assignment increment in `{}` requires two operands",
                            self.path
                        ));
                    };
                    let (lhs, rhs) = (*lhs, *rhs);
                    incr_stmts.push(self.lower_assignment_operands(lhs, rhs, true, op, true)?);
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
                    shortreal: false,
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
                    shortreal: false,
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
    /// Detached branches may retain automatic subroutine storage through an
    /// activation frame.  A synchronous function can only use `join_none`,
    /// while an inlined task body may use the ordinary join variants.
    fn lower_fork(
        &mut self,
        target: Option<NodeId>,
        join_kind: DbJoinKind,
        branches: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        if let Some(function) = &self.func {
            if !function.is_task && join_kind != DbJoinKind::None {
                return Err(format!(
                    "blocking fork/join in function `{}` is not allowed; use join_none",
                    self.path
                ));
            }
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
            k => {
                return Err(format!(
                    "unsupported fork join type {k:?} in `{}`",
                    self.path
                ))
            }
        };
        let target = target
            .map(|target| self.cg.activation_target(target))
            .transpose()?;
        let capture_targets = branches
            .iter()
            .map(|branch| self.cg.fork_capture_targets(*branch))
            .collect::<Vec<_>>();
        let has_captures = capture_targets.iter().any(|targets| !targets.is_empty());
        let enclosing_func = self.func.clone();
        let detached_function_branch = enclosing_func
            .as_ref()
            .is_some_and(|function| !function.is_task && function.def_node.is_some())
            || self.inline.is_some();
        let process_kind = self.process_kind;
        let mut names = Vec::with_capacity(branches.len());
        let mut captured_branches = Vec::with_capacity(branches.len());
        for (k, branch) in branches.iter().enumerate() {
            let fn_name = format!("{}_b{}", self.cg.new_fn_name(&self.path, "fork"), k);
            let label = format!("{}.br{k}", self.path);
            if !has_captures {
                // Branches live in the same instance scope: same path, same
                // owning instance; refs resolve to the instance globals.
                let saved_cg_func = self.cg.func.clone();
                let saved_cg_depth = self.cg.depth_arg.clone();
                let (body_result, mut nested_pre_fns) = {
                    let mut bctx = EmitCtx::new(
                        self.cg,
                        self.path.clone(),
                        self.inst,
                        "0",
                        (!detached_function_branch)
                            .then(|| enclosing_func.clone())
                            .flatten(),
                        None,
                        false,
                    );
                    bctx.process_kind = process_kind;
                    let body_result = bctx.lower_stmt(*branch);
                    (body_result, std::mem::take(&mut bctx.pre_fns))
                };
                self.cg.func = saved_cg_func;
                self.cg.depth_arg = saved_cg_depth;
                let body = body_result?;
                self.pre_fns.append(&mut nested_pre_fns);
                self.pre_fns.push(crate::sim::ir::IrPreFn::Branch {
                    c_name: fn_name.clone(),
                    body,
                });
                names.push((fn_name, label));
                continue;
            }

            let frame = self.cg.new_frame_id()?;
            let previous_captures = self.cg.capture_locals.clone();
            let mut captures = Vec::with_capacity(capture_targets[k].len());
            for (slot, target) in capture_targets[k].iter().enumerate() {
                let source = self.cg.capture_source(*target).ok_or_else(|| {
                    format!(
                        "automatic declaration `{}` was not collected before fork capture in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    )
                })?;
                let initial = previous_captures
                    .get(target)
                    .map(|binding| {
                        IrExpr::new(
                            IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                            binding.local.width,
                            binding.local.signed,
                            None,
                        )
                    })
                    .unwrap_or(source.initial);
                let storage = StorageRef::for_declaration(
                    frame,
                    slot as u32,
                    self.cg.declaration_identity(*target)?,
                    source.lifetime,
                    StorageOwnership::Owned,
                )
                .with_kind(super::storage_kind(source.info.width));
                let local = ProcLocalInfo {
                    c_name: Codegen::capture_local_name(storage),
                    width: source.info.width,
                    signed: source.info.signed,
                    two_state: source.info.two_state,
                    static_signal: None,
                };
                self.cg
                    .capture_locals
                    .insert(*target, CaptureBinding { storage, local });
                captures.push(IrCapture::new(storage, initial));
            }

            let saved_cg_func = self.cg.func.clone();
            let saved_cg_depth = self.cg.depth_arg.clone();
            let (body_result, mut nested_pre_fns) = {
                let mut bctx = EmitCtx::new(
                    self.cg,
                    self.path.clone(),
                    self.inst,
                    "0",
                    (!detached_function_branch)
                        .then(|| enclosing_func.clone())
                        .flatten(),
                    None,
                    false,
                );
                bctx.process_kind = process_kind;
                let body_result = bctx.lower_stmt(*branch);
                (body_result, std::mem::take(&mut bctx.pre_fns))
            };
            self.cg.func = saved_cg_func;
            self.cg.depth_arg = saved_cg_depth;
            self.cg.capture_locals = previous_captures;
            let body = body_result?;
            self.pre_fns.append(&mut nested_pre_fns);
            self.pre_fns.push(crate::sim::ir::IrPreFn::CapturedBranch {
                c_name: fn_name.clone(),
                frame,
                captures: captures.clone(),
                body,
            });
            names.push((fn_name, label));
            let (branch_name, branch_label) = names
                .last()
                .cloned()
                .expect("captured branch name was just pushed");
            captured_branches.push(IrCapturedBranch::new(
                branch_name,
                branch_label,
                frame,
                captures,
            ));
        }
        // A fork is a wait even for join_none: a wait-free `always` containing
        // `fork … join_none` must not be wrapped as a comb process (its
        // branches are child coroutines, not combinational re-evaluation).
        self.saw_wait = true;
        if has_captures {
            Ok(vec![IrStmt::CapturedFork {
                join_kind: join,
                branches: captured_branches,
                target,
            }])
        } else {
            Ok(vec![IrStmt::Fork {
                join_kind: join,
                branches: names,
                target,
            }])
        }
    }

    /// Lower `force lhs = rhs;` into a live runtime binding. The target keeps
    /// its canonical assignment shape so selected and concatenated net
    /// targets can be overlaid without losing their underlying drivers.
    fn lower_force(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Force { lhs, rhs }) = self.cg.kind(h) else {
            return Err("expected a typed force statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let (lhs, rhs) = (*lhs, *rhs);
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "force target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if self.cg.nested_proc_local_ref(rhs).is_some() {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture an automatic procedural local"
            ));
        }
        if let Some(target) = self.nested_subroutine_auto_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if let Some(target) = self.cg.nested_capture_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture activation storage `{}`",
                self.cg.node(target).name
            ));
        }
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let target_real = validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        let rhs_ir = self
            .cg
            .lower_expr(&self.path, rhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let value = if target_real {
            let IrLhs::Whole(index) = &lh else {
                return Err(format!(
                    "real force target in `{diagnostic_path}` must be a whole variable"
                ));
            };
            let shortreal = match self.cg.model.signal(*index).ty {
                IrType::Real { shortreal } => shortreal,
                _ => false,
            };
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(rhs_ir),
                    shortreal,
                },
                0,
                true,
                None,
            )
        } else {
            let width = packed_lhs_width(&self.cg.model, &lh)
                .ok_or_else(|| format!("force target in `{}` has no packed width", self.path))?;
            let signed = force_lhs_signed(&self.cg.model, &lh);
            let rhs_ir = apply_assignment_expression_width(rhs_ir, width);
            // Force uses the same value-preserving RHS-to-target conversion as
            // ordinary assignments (LRM §10.7). Per-part two-state coercion is
            // applied by the runtime when a force spans a concatenation.
            ir_to_vector(rhs_ir, width, signed)?
        };
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: value.clone(),
            real: target_real,
        });
        let read_names = self.cg.collect_force_read_signals(&diagnostic_path, rhs)?;
        let mut reads = Vec::with_capacity(read_names.len());
        for name in read_names {
            let index = self
                .cg
                .model
                .signals
                .iter()
                .position(|signal| signal.c_name == name)
                .ok_or_else(|| {
                    format!(
                        "force dependency `{name}` in `{diagnostic_path}` has no lowered storage"
                    )
                })?;
            if !reads.contains(&index) {
                reads.push(index);
            }
        }
        Ok(IrStmt::Force {
            lhs: lh,
            value,
            eval,
            reads,
        })
    }

    /// Lower `release lhs;` against the same canonical target descriptor used
    /// by `force`, so selected nets release only their matching overlay.
    fn lower_release(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Release { lhs }) = self.cg.kind(h) else {
            return Err("expected a typed release statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let lhs = *lhs;
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "release target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        Ok(IrStmt::Release { lhs: lh })
    }

    fn nested_subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.cg.subroutine_auto_ref(node) {
            return Some(target);
        }
        self.cg
            .node(node)
            .children
            .iter()
            .find_map(|child| self.nested_subroutine_auto_ref(*child))
    }

    fn force_error(&self, diagnostic_path: &str, error: String) -> String {
        if diagnostic_path == self.path {
            error
        } else {
            format!("{error} (source location: {diagnostic_path})")
        }
    }

    fn force_diagnostic_path(&self, node: NodeId) -> String {
        let source = self.cg.node(node);
        match (source.file.as_deref(), source.line, source.col) {
            (Some(file), line, col) if line > 0 => {
                format!("{} at {file}:{line}:{col}", self.path)
            }
            _ => self.path.clone(),
        }
    }

    /// Collect the plain source nodes that make up a legal ordinary packed
    /// concatenation target. Streaming targets are a different language
    /// construct and remain outside procedural continuous assignment support.
    fn pca_source_nodes(&self, lhs: NodeId, out: &mut Vec<NodeId>) -> Result<(), String> {
        match self.cg.kind(lhs) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                reordered,
                operands,
                ..
            }) => {
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                for operand in operands {
                    self.pca_source_nodes(operand, out)?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::Streaming { .. }) => Err(format!(
                "procedural continuous assignment on a streaming target in `{}` is not \
                 supported",
                self.path
            )),
            _ => {
                out.push(lhs);
                Ok(())
            }
        }
    }

    fn pca_lhs_parts(
        &self,
        lhs: &IrLhs,
        out: &mut Vec<usize>,
        stmt: &str,
    ) -> Result<(), String> {
        match lhs {
            IrLhs::Whole(index) => out.push(*index),
            IrLhs::Stream { parts, .. } => {
                for (part, _) in parts {
                    self.pca_lhs_parts(part, out, stmt)?;
                }
            }
            _ => {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables and ordinary concatenations only)",
                    self.path
                ));
            }
        }
        Ok(())
    }

    /// Resolve the target of a procedural continuous `assign` / `deassign`.
    /// Variables only (LRM 1364-1995 §9.4): nets, selects/part-selects/
    /// array elements, hierarchical paths and streaming targets are rejected
    /// cleanly. Whole real variables and ordinary packed concatenations are
    /// represented as several target bindings sharing one site identity.
    fn pca_targets(&mut self, lhs: NodeId, stmt: &str) -> Result<Vec<(usize, SignalInfo)>, String> {
        let mut source_nodes = Vec::new();
        self.pca_source_nodes(lhs, &mut source_nodes)?;
        for source in &source_nodes {
            if matches!(self.cg.kind(*source), NodeKind::Expr(ExprKind::HierPath { .. })) {
                return Err(format!(
                    "procedural continuous `{stmt}` on a hierarchical target in `{}` is not \
                     supported (variables of the current scope only)",
                    self.path
                ));
            }
            // Net targets: the elaborated ref normally binds the declaration,
            // so the net/var distinction is read straight off the arena node.
            // A module-level `reg` can be captured as a net node, but its
            // semantic net type still identifies it as variable storage.
            if let NodeKind::Expr(ExprKind::Ref { target: Some(target) }) = self.cg.kind(*source)
            {
                if let NodeKind::Net { net_type, .. } = self.cg.kind(*target) {
                    if *net_type != NetType::Reg {
                        return Err(format!(
                            "procedural continuous `{stmt}` on net `{}` in `{}` is not supported \
                             (variables only)",
                            self.cg.node(*target).name,
                            self.path
                        ));
                    }
                }
            }
            if !matches!(
                self.cg.kind(*source),
                NodeKind::Var { .. } | NodeKind::Expr(ExprKind::Ref { .. })
            ) {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables and ordinary concatenations only)",
                    self.path
                ));
            }
        }

        let lowered = self.cg.lower_lhs(&self.path, lhs)?;
        let mut indices = Vec::new();
        self.pca_lhs_parts(&lowered, &mut indices, stmt)?;
        if indices.len() != source_nodes.len() {
            return Err(format!(
                "procedural continuous `{stmt}` target shape in `{}` is not supported",
                self.path
            ));
        }
        let mut targets = Vec::with_capacity(indices.len());
        for sig_idx in indices {
            let info = self
                .cg
                .signals
                .iter()
                .find(|info| info.ir == sig_idx)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "cannot collect `{}` in `{}` as a procedural continuous assignment \
                         target (whole variables of the current scope only)",
                        self.cg.node(lhs).name,
                        self.path
                    )
                })?;
            targets.push((sig_idx, info));
        }
        if targets.len() > 1 && targets.iter().any(|(_, info)| info.real) {
            return Err(format!(
                "procedural continuous `{stmt}` concatenation in `{}` must contain only \
                 packed variables",
                self.path
            ));
        }
        Ok(targets)
    }

    /// Pre-scan phase 1 for one process body: resolve each collected
    /// ProcContAssign target and allocate its site (enable signal) without
    /// lowering anything. Distinct sites may target the same variable; the
    /// runtime replaces the active binding when one executes.
    pub(super) fn claim_pca_sites(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        for n in nodes {
            let NodeKind::Stmt(StmtKind::ProcContAssign { lhs, .. }) = self.cg.kind(*n) else {
                return Err("expected a typed procedural continuous assignment".to_string());
            };
            let lhs = *lhs;
            let targets = self.pca_targets(lhs, "assign")?;
            let existing = targets.iter().find_map(|(sig_idx, _)| {
                self.cg
                    .pca_sites
                    .get(&(*n, *sig_idx))
                    .map(|site| (site.en, site.site))
            });
            let (en, site) = existing.unwrap_or_else(|| {
                let site = self.cg.pca_seq;
                let en = self.cg.new_pca_enable(&self.path);
                (en, site)
            });
            for (sig_idx, _) in targets {
                self.cg.pca_sites.entry((*n, sig_idx)).or_insert(PcaSite {
                    en,
                    site,
                    guarded_by: None,
                });
            }
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
    /// The statement activates its runtime binding and performs an immediate
    /// write (so the value updates in the same delta); `deassign` removes the
    /// binding and the variable KEEPS its last value (LRM). Distinct sites
    /// share one target-level binding, so a later execution replaces the
    /// earlier site.
    ///
    /// NBA-vs-PCA interplay: while a site is enabled, ordinary procedural
    /// writes to the variable — blocking AND non-blocking — are suppressed by
    /// the runtime; the guard never wakes on changes of the TARGET itself.
    ///
    /// Sites are pre-allocated by [`Codegen::prescan_pca_sites`] before any
    /// body lowers; this method claims the pre-allocated site by materializing
    /// its guard process.
    fn lower_proc_cont_assign(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::ProcContAssign { lhs, rhs }) = self.cg.kind(h) else {
            return Err("expected a typed procedural continuous assignment".to_string());
        };
        let (lhs, rhs) = (*lhs, *rhs);
        if self.func.is_some() || self.inline.is_some() {
            return Err(format!(
                "procedural continuous assignment in `{}` cannot escape a function or task activation",
                self.path
            ));
        }
        if let Some(target) = self.cg.nested_capture_ref(rhs) {
            return Err(format!(
                "procedural continuous assignment in `{}` cannot capture activation storage `{}`",
                self.path,
                self.cg.node(target).name
            ));
        }
        let targets = self.pca_targets(lhs, "assign")?;
        let (first_sig, first_info) = targets.first().cloned().ok_or_else(|| {
            format!(
                "procedural continuous assignment in `{}` has no target",
                self.path
            )
        })?;

        // The dedicated guard process: waits on the RHS read set ∪ {en} and
        // re-writes the CURRENT rhs whenever it wakes while enabled.
        let mut sens = Vec::new();
        let mut seen = HashSet::new();
        let mut visited = HashSet::new();
        self.cg
            .walk_read_signals(&self.path, rhs, &mut seen, &mut visited, &mut sens)?;
        let rhs_ir = self.cg.lower_expr(&self.path, rhs)?;
        let values = if first_info.real {
            if targets.len() != 1 {
                return Err(format!(
                    "procedural continuous assignment target in `{}` mixes real and packed values",
                    self.path
                ));
            }
            vec![(first_sig, rhs_ir)]
        } else {
            let total_width = targets.iter().try_fold(0u32, |width, (_, info)| {
                width.checked_add(info.width)
            }).ok_or_else(|| {
                format!(
                    "procedural continuous assignment target width overflows in `{}`",
                    self.path
                )
            })?;
            let rhs_ir = apply_assignment_expression_width(rhs_ir, total_width);
            let rhs_ir = ir_to_storage(rhs_ir, total_width, false, false)?;
            let mut cursor = total_width;
            let mut values = Vec::with_capacity(targets.len());
            for (sig_idx, info) in &targets {
                let right = cursor.checked_sub(info.width).ok_or_else(|| {
                    format!(
                        "procedural continuous assignment target width underflows in `{}`",
                        self.path
                    )
                })?;
                let part = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(rhs_ir.clone()),
                        left: i64::from(cursor - 1),
                        right: i64::from(right),
                    },
                    info.width,
                    false,
                    None,
                );
                values.push((
                    *sig_idx,
                    ir_to_storage(part, info.width, info.signed, info.two_state)?,
                ));
                cursor = right;
            }
            values
        };
        // Site bookkeeping. Sites exist for every process-body statement
        // already (pre-scan); a missing entry is only reachable from trees
        // the pre-scan does not walk (defensive fallback with identical
        // semantics). A delay-bearing task body can be lowered at several
        // call sites, so its one guard and site identity are reused.
        let existing = self
            .cg
            .pca_sites
            .get(&(h, first_sig))
            .map(|s| (s.en, s.site, s.guarded_by));
        let (en_ir, site_id) = match existing {
            Some((en, site, Some(_))) => {
                return Ok(values
                    .into_iter()
                    .map(|(sig, value)| IrStmt::PcaAssign {
                        sig,
                        enable: en,
                        site,
                        value,
                    })
                    .collect());
            }
            Some((en, site, None)) => {
                for (sig, _) in &targets {
                    if let Some(pca) = self.cg.pca_sites.get_mut(&(h, *sig)) {
                        pca.guarded_by = Some(h);
                    }
                }
                (en, site)
            }
            None => {
                let site = self.cg.pca_seq;
                let en = self.cg.new_pca_enable(&self.path);
                for (sig, _) in &targets {
                    self.cg.pca_sites.insert(
                        (h, *sig),
                        PcaSite {
                            en,
                            site,
                            guarded_by: Some(h),
                        },
                    );
                }
                (en, site)
            }
        };
        let en_global = self.cg.model.signals[en_ir].c_name.clone();
        let en_dependency = crate::sim::ir::IrDependency::scalar(en_global);
        if !sens.contains(&en_dependency) {
            sens.push(en_dependency);
        }
        let drives = values
            .iter()
            .map(|(sig, value)| IrStmt::PcaDrive {
                sig: *sig,
                enable: en_ir,
                site: site_id,
                value: value.clone(),
            })
            .collect();
        let guard_body = vec![
            IrStmt::WaitAny { sens },
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(en_ir), 1, false, None),
                then_: drives,
                els: None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "pca");
        let origin = self.cg.origin(h);
        self.cg.model.processes.push(IrProcess::new_with_origin(
            guard_name,
            format!("{}.pca", self.path),
            IrShape::Loop,
            Vec::new(),
            guard_body,
            origin,
        ));

        // Statement execution activates/replaces the binding and immediately
        // drives the target. The runtime drops the target write while forced
        // but retains the evaluated RHS for release.
        Ok(values
            .into_iter()
            .map(|(sig, value)| IrStmt::PcaAssign {
                sig,
                enable: en_ir,
                site: site_id,
                value,
            })
            .collect())
    }

    /// Lower `deassign <variable>;` — remove the target's active procedural
    /// continuous assignment binding. The variable KEEPS its last assigned
    /// value (LRM 1364-1995 §9.4). Deassign before any assign is harmless;
    /// net/select/hierarchical targets are rejected like `assign` targets.
    fn lower_deassign(&mut self, lhs: NodeId) -> Result<Vec<IrStmt>, String> {
        Ok(self
            .pca_targets(lhs, "deassign")?
            .into_iter()
            .map(|(sig, _)| IrStmt::PcaDeassign { sig })
            .collect())
    }

    /// Lower system-task calls ($display/$monitor/$strobe/$finish/…).
    /// Skippable constructs warn here and produce no statements.
    fn lower_dumpvars(&mut self, args: &[NodeId]) -> Result<IrStmt, String> {
        let (depth, first_selection) = match args.first().copied() {
            Some(first) => match self.cg.eval_bound_i128(first) {
                Ok(value) => {
                    let depth = u32::try_from(value).map_err(|_| {
                        format!(
                            "$dumpvars depth must be a non-negative 32-bit constant in `{}`",
                            self.path
                        )
                    })?;
                    (depth, 1usize)
                }
                Err(_) => (0, 0usize),
            },
            None => (0, 0usize),
        };
        let mut names = args[first_selection..]
            .iter()
            .map(|argument| self.cg.waveform_selection_name(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        // `$dumpvars(depth)` uses the current elaborated module instance as
        // its implicit scope.  Keep this as owned hierarchy metadata rather
        // than converting the textual process path at runtime.
        if names.is_empty() && depth != 0 {
            names.push(self.cg.waveform_name_for(self.inst));
        }
        Ok(IrStmt::WaveDumpVars(crate::sim::ir::IrWaveDumpVars::new(
            depth, names,
        )))
    }

    fn lower_sys_call(&mut self, h: NodeId, name: &str) -> Result<Vec<IrStmt>, String> {
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        if let Some((task_kind, default_radix)) = display_task_variant(name) {
            match task_kind {
                DisplayTaskKind::Immediate { newline } => {
                    let (fmt, display_args) =
                        self.parse_display_call(name, &args, default_radix)?;
                    return Ok(vec![IrStmt::DisplayTyped {
                        fmt,
                        args: display_args,
                        scope: self.path.clone(),
                        newline,
                        default_radix,
                    }]);
                }
                DisplayTaskKind::Deferred { strobe } => {
                    if self.func.is_some() || self.inline.is_some() {
                        return Err(format!(
                            "{name} in `{}` cannot escape a function or task activation",
                            self.path
                        ));
                    }
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
                    if let Some(local) =
                        args.iter().find_map(|arg| self.cg.nested_capture_ref(*arg))
                    {
                        return Err(format!(
                            "{name} in `{}` cannot defer a reference to activation storage `{}`",
                            self.path,
                            self.cg.node(local).name
                        ));
                    }
                    if self.in_final {
                        return Err(format!(
                            "{name} inside a final block in `{}` is not supported: \
                             no scheduled output events execute after final procedures",
                            self.path
                        ));
                    }
                    let (fmt, display_args) =
                        self.parse_display_call(name, &args, default_radix)?;
                    let reads = if !strobe {
                        self.collect_monitor_reads(&args)?
                    } else {
                        Vec::new()
                    };
                    // Generated re-evaluator: reads the CURRENT values of the
                    // displayed arguments each time the runtime prints (after an
                    // NBA commit for the monitor, at the end of the time step for
                    // $strobe).  Attached ahead of the enclosing function/process.
                    let eval_name = self.cg.new_fn_name(&self.path, "mon");
                    self.pre_fns.push(crate::sim::ir::IrPreFn::DisplayEval {
                        c_name: eval_name.clone(),
                        args: display_args.clone(),
                    });
                    return Ok(vec![IrStmt::MonitorSet {
                        strobe,
                        fmt,
                        eval: eval_name,
                        n_args: display_args.len(),
                        reads,
                        default_radix,
                        scope: self.path.clone(),
                    }]);
                }
            }
        }
        match name {
            "$monitoron" => Ok(vec![IrStmt::MonitorEnable(true)]),
            "$monitoroff" => Ok(vec![IrStmt::MonitorEnable(false)]),
            "$dumpfile" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumpfile requires exactly one literal string argument in `{}`",
                        self.path
                    ));
                }
                let path = self
                    .literal_string(args[0], "$dumpfile path")?
                    .ok_or_else(|| {
                        format!(
                            "$dumpfile requires a literal string argument in `{}`",
                            self.path
                        )
                    })?;
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
                self.cg.model.waveform = true;
                Ok(vec![self.lower_dumpvars(&args)?])
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
            "$finish" => {
                // Slang's FinishControlTask enforces the target-edition
                // finish_number contract with constant evaluation. Keep the
                // validated level concrete in the IR; runtime expressions
                // are not legal finish arguments in either selected edition.
                let verbosity = match args.as_slice() {
                    [] => 1,
                    [argument] => {
                        let value = self.cg.eval_bits(*argument).map_err(|error| {
                            format!(
                                "$finish argument at {} must be an integral constant 0, 1, or 2: {error}",
                                self.finish_location(h)
                            )
                        })?;
                        if value.is_unknown() {
                            return Err(format!(
                                "$finish argument at {} must be a known integral constant 0, 1, or 2",
                                self.finish_location(h)
                            ));
                        }
                        let value = value.to_u128().ok_or_else(|| {
                            format!(
                                "$finish argument at {} must be an integral constant 0, 1, or 2",
                                self.finish_location(h)
                            )
                        })?;
                        u8::try_from(value)
                            .ok()
                            .filter(|value| *value <= 2)
                            .ok_or_else(|| {
                                format!(
                                    "$finish argument at {} must be 0, 1, or 2 (got {value})",
                                    self.finish_location(h)
                                )
                            })?
                    }
                    _ => {
                        return Err(format!(
                            "$finish accepts at most one argument at {}",
                            self.finish_location(h)
                        ));
                    }
                };
                Ok(vec![IrStmt::FinishControl {
                    verbosity,
                    location: self.finish_location(h),
                }])
            }
            "$printtimescale" => {
                let ts = self.cg.timescale_of_node(h);
                Ok(vec![IrStmt::PrintTimescale {
                    unit_fs: ts.unit_fs,
                    precision_fs: ts.precision_fs,
                    label: self.path.clone(),
                }])
            }
            "$displayon" | "$displayoff" => {
                self.cg
                    .warnings
                    .push(format!("{name} in `{}` skipped (not supported)", self.path));
                Ok(vec![])
            }
            _ => Err(format!("unsupported system task {name} in `{}`", self.path)),
        }
    }

    /// Collect the signal storage that can trigger a monitor. The format
    /// string is display metadata, and symbolic time queries have no signal
    /// reads, so only value arguments contribute to this list.
    fn collect_monitor_reads(
        &self,
        args: &[NodeId],
    ) -> Result<Vec<crate::sim::ir::IrDependency>, String> {
        let mut fmt_seen = false;
        let mut reads = Vec::new();
        let mut seen = HashSet::new();
        for arg in args {
            let is_fmt = matches!(
                self.cg.kind(*arg),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    ..
                })
            );
            if is_fmt && !fmt_seen {
                fmt_seen = true;
                continue;
            }
            if self.cg.is_string_expr(&self.path, *arg) {
                continue;
            }
            for read in self.cg.collect_read_signals(&self.path, *arg)? {
                if seen.insert(read.clone()) {
                    reads.push(read);
                }
            }
        }
        Ok(reads)
    }

    /// Parse a display-family call into a C format string and typed values.
    /// Values remain packed, real, or owned strings until the shared runtime
    /// formatter consumes them. A literal first string is the format; when it
    /// is absent each argument gets its family default conversion.
    fn parse_display_call(
        &mut self,
        name: &str,
        args: &[NodeId],
        default_radix: IrDisplayRadix,
    ) -> Result<(String, Vec<crate::sim::ir::IrDisplayArg>), String> {
        let mut fmt_arg: Option<String> = None;
        let mut display_args = Vec::new();
        for a in args {
            let is_fmt = self.literal_string(*a, name)?.is_some() && fmt_arg.is_none();
            if is_fmt && fmt_arg.is_none() {
                fmt_arg = self.literal_string(*a, name)?;
            } else {
                display_args.push(self.lower_display_arg(*a)?);
            }
        }
        let Some(fmt) = fmt_arg else {
            let mut c_fmt = String::from("\"");
            for arg in &display_args {
                c_fmt.push('%');
                c_fmt.push(match arg {
                    crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                    crate::sim::ir::IrDisplayArg::String(_) => 's',
                    crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
                });
            }
            c_fmt.push('"');
            return Ok((c_fmt, display_args));
        };
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
                if matches!(n, '-' | '+' | ' ' | '#' | '0' | '.') || n.is_ascii_digit() {
                    spec.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let Some(conv) = chars.next() else {
                return Err(format!(
                    "incomplete {name} format at end of `{}`",
                    self.path
                ));
            };
            spec.push(conv);
            match conv {
                'd' | 'h' | 'b' | 'o' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(&display_args[arg_idx], crate::sim::ir::IrDisplayArg::Packed(_)) {
                        return Err(format!(
                            "{name} integer format `%{conv}` requires a packed argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push(conv);
                }
                's' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(&display_args[arg_idx], crate::sim::ir::IrDisplayArg::String(_)) {
                        return Err(format!(
                            "{name} format `%s` requires a string argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec);
                }
                'f' | 'e' | 'g' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !matches!(&display_args[arg_idx], crate::sim::ir::IrDisplayArg::Real(_)) {
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
                    if !matches!(&display_args[arg_idx], crate::sim::ir::IrDisplayArg::Packed(_)) {
                        return Err(format!(
                            "{name} format `%t` requires a packed argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec);
                }
                'm' => {
                    // `%m` is a scope query and consumes no value argument.
                    c_fmt.push_str(&spec);
                }
                '%' => c_fmt.push_str(&spec),
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{other}` in `{}`",
                        self.path
                    ))
                }
            }
        }
        while arg_idx < display_args.len() {
            c_fmt.push('%');
            c_fmt.push(match &display_args[arg_idx] {
                crate::sim::ir::IrDisplayArg::Real(_) => 'f',
                crate::sim::ir::IrDisplayArg::String(_) => 's',
                crate::sim::ir::IrDisplayArg::Packed(_) => default_radix.specifier(),
            });
            arg_idx += 1;
        }
        c_fmt.push('"');
        Ok((c_fmt, display_args))
    }

    fn lower_display_arg(
        &mut self,
        node: NodeId,
    ) -> Result<crate::sim::ir::IrDisplayArg, String> {
        if self.cg.is_string_expr(&self.path, node) {
            return Ok(crate::sim::ir::IrDisplayArg::String(
                self.cg.lower_string(&self.path, node)?,
            ));
        }
        let value = self.cg.lower_expr(&self.path, node)?;
        Ok(if value.is_real() {
            crate::sim::ir::IrDisplayArg::Real(value)
        } else {
            crate::sim::ir::IrDisplayArg::Packed(value)
        })
    }

    /// Recover a source string literal through the implicit string cast that
    /// Slang may insert at a system-task argument boundary.
    fn literal_string(&self, node: NodeId, context: &str) -> Result<Option<String>, String> {
        match self.cg.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => decoded_string_text(value, context).map(Some),
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                self.literal_string(*operand, context)
            }
            _ => Ok(None),
        }
    }

    fn finish_location(&self, node: NodeId) -> String {
        let source = self.cg.node(node);
        if source.line == 0 {
            return self.path.clone();
        }
        format!("{}:{}:{}", self.path, source.line, source.col)
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
        let (ft, callee_inst) = self
            .cg
            .resolve_callee_env(self.inst, name, is_task, callee)?;
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
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        let bound = self.cg.bind_call_args(self.inst, &formals, &args)?;
        let has_event_formal = bound.iter().any(|argument| argument.is_event);
        if has_event_formal {
            return self.lower_task_inline(ft, callee_inst, h, &formals, &bound);
        }
        if is_task
            && (self.cg.task_has_disable(ft, callee_inst)
                || self.cg.task_is_disable_target(ft))
        {
            // Named disable must unwind the callee's activation before any
            // caller-side copy-out. Keep that path inline until task returns
            // carry an explicit cancellation result in the C ABI. The
            // declaration-level target check covers callers that disable a
            // task externally rather than from inside the task body.
            self.lower_task_inline(ft, callee_inst, h, &formals, &bound)
        } else {
            let fidx = self
                .cg
                .func_meta
                .get(&ft)
                .map(|m| m.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            self.lower_call_stmts(fidx, callee_inst, h, &formals, &bound)
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
            if matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg { ty, .. } if ty.kind == "chandle"
            ) {
                let is_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    let (target, _) = self.cg.lower_chandle_lvalue(&self.path, bound[idx].expr)?;
                    let address = self.cg.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                } else {
                    in_args.push(IrCallArg::ChandleVal(
                        self.cg.lower_chandle(&self.path, bound[idx].expr)?,
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
                    if !const_ref {
                        self
                            .cg
                            .ensure_string_actual_writable(&self.path, bound[idx].expr)?;
                    }
                    out_args.push(IrCallArg::StringRefAddr {
                        addr: self
                            .cg
                            .lower_string_actual_address(&self.path, bound[idx].expr)?,
                        const_ref,
                    });
                } else {
                    out_args.push(self.cg.lower_ref_arg(
                        &self.path,
                        &bound[idx],
                        const_ref,
                        ref_static,
                    )?);
                }
                if !bound[idx].string {
                    let read_ir = self.cg.lower_expr(&self.path, bound[idx].expr)?;
                    arg_codes[idx] = Some(self.cg.render_ir_code(&read_ir)?);
                    arg_irs[idx] = Some(read_ir);
                }
                continue;
            }
            if !*is_out {
                continue;
            }
            if bound[idx].string {
                self
                    .cg
                    .ensure_string_actual_writable(&self.path, bound[idx].expr)?;
                let writeback = self
                    .cg
                    .lower_string_actual_address(&self.path, bound[idx].expr)?;
                let init = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg { direction: DbDirection::Inout, .. }
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
            let (lh, actual_read, selector_inits) = self.cg.lower_call_actual(
                &self.path,
                bound[idx].expr,
                &format!("{}_{}", h.0, idx),
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
            if let Some(storage) = self.cg.static_formals.get(&(callee_inst, *io)).cloned() {
                let storage_lhs = IrLhs::Whole(storage.ir);
                if matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                ) {
                    let value = actual_read.clone();
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
            let (_init_code, init_ir) = self
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
            arg_codes[idx] = Some(tname.clone());
            out_args.push(IrCallArg::OutAddr(format!("&{tname}")));
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let is_ref = matches!(
                self.cg.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !*is_out
                && !is_ref
                && matches!(self.cg.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "chandle")
            {
                continue;
            }
            if !*is_out && !is_ref {
                if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.cg.lower_string(&self.path, bound[idx].expr)?,
                    ));
                } else {
                    let (_code, ir) = self.cg.lower_bound_arg_code(
                        &self.path,
                        formals,
                        bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(ir));
                }
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

    /// Lower a cancellation/event-bearing task body inlined at its call site: the task's
    /// io_decls are bound to the caller's argument expressions (writes go
    /// straight to the bound actuals through the func-context remap), locals
    /// get fresh names, and the body lowers under the inline context. This
    /// path deliberately remains bounded to cases that need activation
    /// rebinding; resumable timed calls use the typed `IrFunc` path above.
    fn lower_task_inline(
        &mut self,
        ft: NodeId,
        callee_inst: NodeId,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<IrStmt, String> {
        let tname = self.cg.node(ft).name.clone();
        let activation_target = self.cg.activation_target(ft)?;
        let automatic = matches!(
            self.cg.kind(ft),
            NodeKind::FuncTask {
                automatic: true,
                ..
            }
        );
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

        // Resolve each declaration's lifetime independently: automatic locals
        // get fresh C names per inline site, while static locals map to
        // model-global storage below even in an automatic task.
        let mut locals: HashMap<NodeId, (String, u32, bool, bool, bool)> = HashMap::new();
        let mut chandle_locals: HashMap<NodeId, String> = HashMap::new();
        let mut local_seq = 0usize;
        let prefix = format!("_i{}", h.0);
        self.cg
            .collect_func_locals(
                body,
                callee_inst,
                &mut locals,
                &mut chandle_locals,
                &mut local_seq,
                &prefix,
            )?;

        // Formals bound to the caller's argument expressions.
        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut event_args: HashMap<NodeId, IrEventRef> = HashMap::new();
        let mut arg_dependencies: HashMap<NodeId, Vec<IrDependency>> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        let mut arg_lhs: HashMap<NodeId, Lhs> = HashMap::new();
        let mut const_refs: HashSet<NodeId> = HashSet::new();
        let mut const_ref_lhs: HashMap<NodeId, Lhs> = HashMap::new();
        let mut persistent = HashMap::new();
        let mut chandle_read = HashMap::new();
        let mut chandle_write = HashMap::new();
        let mut string_read = HashMap::new();
        let mut string_write = HashMap::new();
        let mut string_addr = HashMap::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut string_cleanups = Vec::new();
        // Automatic input formals bound to caller rvalue expressions need a
        // writable local copy. Static formals use their persistent signal.
        let mut input_copies: Vec<(String, IrExpr, bool)> = Vec::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let b = &bound[idx];
            if b.is_event {
                let output_only = *is_out
                    && matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Output,
                            ..
                        }
                    );
                let actual = (!output_only && !self.cg.is_null_event_expression(b.expr))
                    .then(|| self.cg.event_target_of(b.expr))
                    .flatten();
                let event = if let Some(target) = actual.as_ref() {
                    self.cg.event_ref_of(target, &self.path)?
                } else {
                    IrEventRef::Null
                };
                if !output_only && actual.is_none() && !self.cg.is_null_event_expression(b.expr) {
                    return Err(format!(
                        "event actual for formal `{}` in task `{tname}` is not a named event (node kind: {:?})",
                        self.cg.node(*io).name,
                        self.cg.kind(b.expr)
                    ));
                }
                let captured = format!("_ievent_{}_{}", h.0, idx);
                before.push(IrStmt::EventCapture {
                    name: captured.clone(),
                    source: event,
                });
                if *is_out {
                    let Some(target) = actual else {
                        return Err(format!(
                            "event output formal `{}` in task `{tname}` requires an event actual",
                            self.cg.node(*io).name
                        ));
                    };
                    let target = self.cg.event_ref_of(&target, &self.path)?;
                    after.push(IrStmt::EventAssign {
                        target,
                        source: Some(IrEventRef::Captured(captured.clone())),
                    });
                }
                event_args.insert(*io, IrEventRef::Captured(captured));
                continue;
            }
            if matches!(self.cg.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "chandle") {
                let is_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref: true,
                        ..
                    }
                );
                if is_ref || *is_out {
                    let (target, read) = self.cg.lower_chandle_lvalue(&self.path, b.expr)?;
                    chandle_read.insert(*io, read);
                    if !const_ref {
                        chandle_write.insert(*io, target);
                    }
                } else {
                    let cname = format!("_il{}_{}", h.0, idx);
                    let value = self.cg.lower_chandle(&self.path, b.expr)?;
                    before.push(IrStmt::Object(IrObjectStmt::ChandleDeclareLocal(
                        cname.clone(),
                        Some(value),
                    )));
                    chandle_read.insert(*io, IrChandleExpr::LocalRead(cname.clone()));
                    chandle_write.insert(*io, ChandleTarget::Local(cname));
                }
                continue;
            }
            if b.string {
                let is_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg { const_ref: true, .. }
                );
                if is_ref {
                    if !const_ref {
                        self
                            .cg
                            .ensure_string_actual_writable(&self.path, b.expr)?;
                    }
                    let address = self
                        .cg
                        .lower_string_actual_address(&self.path, b.expr)?;
                    let target = address.trim_start_matches('&').to_owned();
                    string_read.insert(*io, self.cg.lower_string(&self.path, b.expr)?);
                    string_addr.insert(*io, target.clone());
                    if !const_ref {
                        string_write.insert(*io, target);
                    }
                } else if let Some(object) = (!automatic)
                    .then(|| self.cg.static_string_formals.get(&(callee_inst, *io)).copied())
                    .flatten()
                {
                    let name = self.cg.model.objects[object].c_name.clone();
                    string_read.insert(*io, IrStringExpr::Read(object));
                    string_addr.insert(*io, name.clone());
                    string_write.insert(*io, name);
                    if !*is_out
                        || matches!(
                            self.cg.kind(*io),
                            NodeKind::FuncArg {
                                direction: DbDirection::Inout,
                                ..
                            }
                        )
                    {
                        before.push(IrStmt::Object(IrObjectStmt::StringAssign(
                            object,
                            self.cg.lower_string(&self.path, b.expr)?,
                        )));
                    }
                    if *is_out {
                        self
                            .cg
                            .ensure_string_actual_writable(&self.path, b.expr)?;
                        let actual = self
                            .cg
                            .lower_string_actual_address(&self.path, b.expr)?
                            .trim_start_matches('&')
                            .to_owned();
                        after.push(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                            actual,
                            IrStringExpr::Read(object),
                        )));
                    }
                } else if *is_out {
                    self
                        .cg
                        .ensure_string_actual_writable(&self.path, b.expr)?;
                    let cname = format!("_is{}_{}", h.0, idx);
                    let init = if matches!(
                        self.cg.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    ) {
                        Some(self.cg.lower_string(&self.path, b.expr)?)
                    } else {
                        None
                    };
                    before.push(IrStmt::DeclString {
                        name: cname.clone(),
                        init,
                    });
                    let actual = self
                        .cg
                        .lower_string_actual_address(&self.path, b.expr)?
                        .trim_start_matches('&')
                        .to_owned();
                    string_read.insert(*io, IrStringExpr::LocalRead(cname.clone()));
                    string_write.insert(*io, cname.clone());
                    string_addr.insert(*io, cname.clone());
                    after.push(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                        actual,
                        IrStringExpr::LocalRead(cname.clone()),
                    )));
                    string_cleanups.push(cname);
                } else {
                    let cname = format!("_il{}_{}", h.0, idx);
                    before.push(IrStmt::DeclString {
                        name: cname.clone(),
                        init: Some(self.cg.lower_string(&self.path, b.expr)?),
                    });
                    string_read.insert(*io, IrStringExpr::LocalRead(cname.clone()));
                    string_write.insert(*io, cname.clone());
                    string_addr.insert(*io, cname.clone());
                    string_cleanups.push(cname);
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
                let actual_lhs = self.cg.lower_lhs(&self.path, b.expr)?;
                let (actual_lhs, read_ir) = match actual_lhs {
                    IrLhs::ArrayElem {
                        arr,
                        indices,
                        elem_sel: IrElemSel::Whole,
                    } => {
                        let array = self.cg.model.arrays.get(arr).ok_or_else(|| {
                            format!("ref actual array {arr} for task `{tname}` is out of bounds")
                        })?;
                        let mut frozen = Vec::with_capacity(indices.len());
                        for (index_no, index) in indices.into_iter().enumerate() {
                            let name = format!("_ref_idx_{}_{}_{}", h.0, idx, index_no);
                            let (width, signed) = (index.width, index.signed);
                            before.push(IrStmt::DeclLocal {
                                name: name.clone(),
                                width,
                                signed,
                                two_state: false,
                                init: Some(Box::new(index)),
                            });
                            frozen.push(IrExpr::new(
                                IrExprKind::LocalRead(name),
                                width,
                                signed,
                                None,
                            ));
                        }
                        let read = IrExpr::new(
                            IrExprKind::ArrayRead {
                                arr,
                                indices: frozen.clone(),
                                elem_sel: IrElemSel::Whole,
                            },
                            array.elem_width,
                            array.signed,
                            None,
                        );
                        (
                            IrLhs::ArrayElem {
                                arr,
                                indices: frozen,
                                elem_sel: IrElemSel::Whole,
                            },
                            read,
                        )
                    }
                    actual_lhs => {
                        let read_ir = self.cg.lower_expr(&self.path, b.expr)?;
                        (actual_lhs, read_ir)
                    }
                };
                let (width, signed, two_state, actual_const) = self
                    .cg
                    .ref_lhs_type(&actual_lhs)
                    .ok_or_else(|| {
                        format!("ref actual for task `{tname}` is not an integral lvalue")
                    })?;
                if (width, signed, two_state) != (b.width, b.signed, b.two_state) {
                    return Err(format!(
                        "ref actual for task `{tname}` does not exactly match its formal"
                    ));
                }
                let const_ref = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref: true,
                        ..
                    }
                );
                if actual_const && !const_ref {
                    return Err(format!(
                        "const ref actual cannot bind to writable ref formal in `{tname}`"
                    ));
                }
                arg_codes[idx] = Some(self.cg.render_ir_code(&read_ir)?);
                arg_ir.insert(*io, read_ir.clone());
                arg_dependencies.insert(
                    *io,
                    self.cg.collect_read_signals(&self.path, b.expr)?,
                );
                arg_read.insert(
                    *io,
                    ArgMap {
                        width,
                        signed,
                        two_state,
                    },
                );
                if const_ref {
                    const_refs.insert(*io);
                    const_ref_lhs.insert(*io, Lhs::Canonical(actual_lhs));
                } else {
                    arg_lhs.insert(*io, Lhs::Canonical(actual_lhs));
                }
                continue;
            }
            if let Some(storage) = (!automatic)
                .then(|| self.cg.static_formals.get(&(callee_inst, *io)).cloned())
                .flatten()
            {
                let storage_lhs = IrLhs::Whole(storage.ir);
                let storage_read = sig_read_expr_full(&storage);
                arg_write.insert(*io, format!("&{}", storage.global));
                persistent.insert(*io, storage.clone());
                arg_ir.insert(*io, storage_read.clone());
                arg_dependencies.insert(*io, vec![self.cg.signal_dependency(&storage)]);
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: storage.width,
                        signed: storage.signed,
                        two_state: storage.two_state,
                    },
                );
                let is_inout = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                );
                if !*is_out {
                    let (_, value) = self.cg.lower_bound_arg_code(
                        &self.path,
                        formals,
                        bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    before.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.cg.model, &storage_lhs, value),
                        lhs: storage_lhs,
                        nba: false,
                    });
                } else {
                    let (actual_lhs, actual_read, selector_inits) = self.cg.lower_call_actual(
                        &self.path,
                        b.expr,
                        &format!("{}_{}", h.0, idx),
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
                    if is_inout {
                        before.push(IrStmt::Assign {
                            rhs: apply_lhs_assignment_context(
                                &self.cg.model,
                                &storage_lhs,
                                actual_read,
                            ),
                            lhs: storage_lhs,
                            nba: false,
                        });
                    }
                    after.push(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(
                            &self.cg.model,
                            &actual_lhs,
                            storage_read,
                        ),
                        lhs: actual_lhs,
                        nba: false,
                    });
                }
            } else if *is_out {
                let (actual_lhs, actual_read, selector_inits) = self.cg.lower_call_actual(
                    &self.path,
                    b.expr,
                    &format!("{}_{}", h.0, idx),
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
                let cname = format!("_io{}_{}", h.0, idx);
                let is_inout = matches!(
                    self.cg.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Inout,
                        ..
                    }
                );
                let init = if is_inout {
                    self.cg
                        .lower_call_temp_init_from_expr(*io, b, actual_read)?
                        .1
                } else {
                    None
                };
                before.push(IrStmt::DeclLocal {
                    name: cname.clone(),
                    width: b.width,
                    signed: b.signed,
                    two_state: b.two_state,
                    init: init.map(Box::new),
                });
                let read_ir = IrExpr::new(
                    IrExprKind::LocalRead(cname.clone()),
                    b.width,
                    b.signed,
                    None,
                );
                arg_write.insert(*io, format!("&{cname}"));
                arg_ir.insert(*io, read_ir.clone());
                if is_inout {
                    arg_dependencies.insert(
                        *io,
                        self.cg.collect_read_signals(&self.path, b.expr)?,
                    );
                }
                after.push(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(
                        &self.cg.model,
                        &actual_lhs,
                        read_ir.clone(),
                    ),
                    lhs: actual_lhs,
                    nba: false,
                });
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

        for (local, cname) in &chandle_locals {
            match self.cg.db.variable_lifetime(*local) {
                VariableLifetime::Automatic => {
                    chandle_read.insert(*local, IrChandleExpr::LocalRead(cname.clone()));
                    chandle_write.insert(*local, ChandleTarget::Local(cname.clone()));
                }
                VariableLifetime::Static => {}
                VariableLifetime::Unavailable => {
                    return Err(format!(
                        "resolved lifetime is unavailable for task local `{}`",
                        self.cg.node(*local).name
                    ));
                }
            }
        }

        for (local, (cname, ..)) in &locals {
            if matches!(self.cg.kind(*local), NodeKind::Var { ty } if ty.kind == "string")
                && self.cg.db.variable_lifetime(*local) == VariableLifetime::Automatic
            {
                string_read.insert(*local, IrStringExpr::LocalRead(cname.clone()));
                string_write.insert(*local, cname.clone());
                string_addr.insert(*local, cname.clone());
            }
        }

        let persistent_locals = locals
            .keys()
            .copied()
            .filter(|local| self.cg.db.variable_lifetime(*local) == VariableLifetime::Static)
            .collect::<Vec<_>>();
        for local in persistent_locals {
            if matches!(self.cg.kind(local), NodeKind::Var { ty } if ty.kind == "string") {
                let object = self
                    .cg
                    .static_string_task_locals
                    .get(&(callee_inst, local))
                    .copied()
                    .ok_or_else(|| {
                        format!("static string task `{tname}` local has no persistent storage")
                    })?;
                let name = self.cg.model.objects[object].c_name.clone();
                string_read.insert(local, IrStringExpr::Read(object));
                string_write.insert(local, name.clone());
                string_addr.insert(local, name);
                locals.remove(&local);
                continue;
            }
            let storage = self
                .cg
                .static_task_locals
                .get(&(callee_inst, local))
                .cloned()
                .ok_or_else(|| {
                    format!("static task `{tname}` local has no persistent storage")
                })?;
            arg_write.insert(local, format!("&{}", storage.global));
            persistent.insert(local, storage.clone());
            arg_ir.insert(local, sig_read_expr_full(&storage));
            arg_read.insert(
                local,
                ArgMap {
                    width: storage.width,
                    signed: storage.signed,
                    two_state: storage.two_state,
                },
            );
            locals.remove(&local);
        }
        let persistent_chandle_locals = chandle_locals
            .keys()
            .copied()
            .filter(|local| self.cg.db.variable_lifetime(*local) == VariableLifetime::Static)
            .collect::<Vec<_>>();
        for local in persistent_chandle_locals {
            let object = if let Some(object) = self
                .cg
                .static_task_chandle_locals
                .get(&(callee_inst, local))
                .copied()
            {
                object
            } else {
                let object = self.cg.model.objects.len();
                self.cg.model.objects.push(crate::sim::ir::IrObject {
                    c_name: format!(
                        "O_f{}_{}_l{}",
                        callee_inst.index(),
                        ft.index(),
                        local.index()
                    ),
                    ty: crate::sim::ir::IrObjectType::Chandle,
                    initial: None,
                });
                self.cg
                    .static_task_chandle_locals
                    .insert((callee_inst, local), object);
                object
            };
            chandle_read.insert(local, IrChandleExpr::Read(object));
            chandle_write.insert(local, ChandleTarget::Object(object));
            chandle_locals.remove(&local);
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
            event_args,
            arg_dependencies,
            arg_write,
            arg_lhs,
            const_refs,
            const_ref_lhs,
            persistent,
            chandle_read,
            chandle_write,
            string_read,
            string_write,
            string_addr,
            locals,
            ret_node: None,
            // This expansion has no separate C function identity. Runtime
            // activation targets implement disable; only return uses done_label.
            def_node: None,
        };
        let inline = InlineCtx {
            done_label: done_label.clone(),
            chain,
            used: false,
        };

        // Swap in the inline context (and sync the codegen for expression
        // resolution), then restore on the way out.
        let saved_cg = (
            self.cg.func.take(),
            self.cg.depth_arg.clone(),
            self.cg.inst,
        );
        let saved_ctx = (self.func.take(), self.inline.take(), self.depth_arg.clone());
        let depth = format!("({}) + 1", saved_ctx.2);
        self.cg.func = Some(func.clone());
        self.cg.depth_arg = depth.clone();
        self.func = Some(func);
        self.inline = Some(inline);
        self.depth_arg = depth;

        let mut stmts: Vec<IrStmt> = before;
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
        // Actual expressions were lowered in the caller context above. The
        // task body must resolve nested calls, locals, and instance storage
        // in the callee's concrete environment.
        let saved_inst = self.inst;
        self.inst = callee_inst;
        self.cg.inst = callee_inst;
        let task_body_result = self.lower_stmt(body);
        self.inst = saved_inst;
        self.cg.inst = saved_cg.2;
        let mut task_body = task_body_result?;
        match self.ctrl.pop() {
            Some(CtrlScope::TaskBody) => {}
            _ => unreachable!("task-body scope stack imbalance"),
        }
        if self.inline.as_ref().map(|i| i.used).unwrap_or(false) {
            task_body.push(IrStmt::Label(done_label));
        }
        // Copy-out and event-formal rebinding are part of the activation's
        // normal return path. Keeping them inside the scope lets the emitted
        // cancellation guard jump over them when `disable` unwinds a task.
        task_body.extend(after);
        stmts.push(IrStmt::ActivationScope {
            target: activation_target,
            exit: self.new_label("xt"),
            body: task_body,
        });
        for cname in string_cleanups {
            stmts.push(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                cname,
                IrStringExpr::Literal(Vec::new()),
            )));
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
                    IrStmt::Object(crate::sim::ir::IrObjectStmt::StringAssignLocal(
                        "_ret".to_string(),
                        self.cg.lower_string(&self.path, value)?,
                    )),
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

fn force_constant_index(expr: &IrExpr) -> bool {
    let IrExprKind::Const(value) = expr.kind() else {
        return false;
    };
    value.real_value().is_none()
        && value.x_mask().iter().all(|mask| *mask == 0)
        && value.z_mask().iter().all(|mask| *mask == 0)
}

/// Validate the force/release target shape shared by lowering and emission.
/// Packed variable selects are intentionally rejected; a selected net is
/// represented by a fixed part descriptor and can therefore preserve all
/// current driver contributions on release.
fn validate_force_lhs(model: &IrModel, lhs: &IrLhs, path: &str) -> Result<bool, String> {
    match lhs {
        IrLhs::Whole(index) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            Ok(matches!(signal.ty, IrType::Real { .. }))
        }
        IrLhs::WholeRef { .. } => Err(format!(
            "force/release target in `{path}` does not have persistent canonical storage"
        )),
        IrLhs::Ref { .. } => Err(format!(
            "force/release target in `{path}` cannot be a ref formal"
        )),
        IrLhs::Bit(index, select, _) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            if signal.net_driver.is_none() {
                return Err(format!(
                    "force/release of a variable bit-select in `{path}` is not supported"
                ));
            }
            if !force_constant_index(select) {
                return Err(format!(
                    "force/release net bit-select in `{path}` requires a constant index"
                ));
            }
            Ok(false)
        }
        IrLhs::Part(index, ..) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            if signal.net_driver.is_none() {
                return Err(format!(
                    "force/release of a variable part-select in `{path}` is not supported"
                ));
            }
            Ok(false)
        }
        IrLhs::IdxPart(..) => Err(format!(
            "force/release indexed part-select in `{path}` requires a constant net part-select"
        )),
        IrLhs::ArrayElem { .. } => Err(format!(
            "force/release of an unpacked-array element in `{path}` is not supported"
        )),
        IrLhs::Stream {
            parts,
            slice,
            direction,
            ..
        } => {
            let mut real = false;
            for (part, _) in parts {
                if matches!(part, IrLhs::Stream { slice: nested_slice, direction: nested_direction, .. }
                    if *nested_slice != 1
                        || !matches!(nested_direction, IrStreamDirection::LeftToRight))
                {
                    return Err(format!(
                        "nested streaming force/release target in `{path}` is not supported"
                    ));
                }
                if validate_force_lhs(model, part, path)? {
                    real = true;
                }
            }
            if *slice == 0 {
                return Err(format!(
                    "force/release streaming target in `{path}` has zero slice size"
                ));
            }
            let _ = direction;
            if real && parts.len() != 1 {
                return Err(format!(
                    "force/release concatenation containing a real target in `{path}` is not supported"
                ));
            }
            Ok(real)
        }
    }
}

fn force_lhs_signed(model: &IrModel, lhs: &IrLhs) -> bool {
    match lhs {
        IrLhs::Whole(index) => model.signal(*index).ty.signed(),
        IrLhs::WholeRef { signed, .. } => *signed,
        IrLhs::Ref { signed, .. } => *signed,
        _ => false,
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
