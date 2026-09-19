//! Control flow.

use super::*;

impl<'c, 'a> EmitCtx<'c, 'a> {
    /// Lower a loop body under a break/continue scope.  The continue label
    /// is appended to the body's END — for every loop shape that lands on
    /// the next-iteration point (for: the increment step; while/repeat/
    /// forever: the back-edge condition test).  Returns the lowered body and
    /// the trailing break label when any `break` used it.
    pub(super) fn lower_loop_body(
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
    pub(super) fn lower_do_while(
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
            check: IrUniquePriorityCheck::None,
        });
        Ok(vec![IrStmt::Forever { body }, IrStmt::Label(brk)])
    }

    /// Lower `break;` / `continue;`: an atomic jump to the innermost loop's
    /// break/continue label.  Resolution stops at an inlined-task-body
    /// boundary (a `break` can never target a loop of the CALLER) and
    /// outside a loop this is a syntax-level error that the frontend normally
    /// rejects first; kept as a clean codegen reject.
    pub(super) fn lower_break_continue(&mut self, is_break: bool) -> Result<Vec<IrStmt>, String> {
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
    pub(super) fn lower_disable(&mut self, target: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        let target = target
            .ok_or_else(|| format!("cannot resolve the target of `disable` in `{}`", self.path))?;
        Ok(vec![IrStmt::DisableTarget {
            target: self.cg.activation_target(target)?,
        }])
    }
}

impl EmitCtx<'_, '_> {
    pub(super) fn lower_case(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (case_type, check, items) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Case {
                case_type,
                check,
                items,
            }) => (*case_type, *check, items),
            _ => unreachable!("non-case passed to lower_case"),
        };
        let qualifier = lower_unique_priority_check(check, self.cg.origin(h));
        let sel = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "case without selector".to_string())?;
        if case_type == DbCaseKind::Inside && self.cg.is_string_expr(&self.path, sel) {
            let selector = self.cg.lower_string(&self.path, sel)?;
            return self.lower_case_inside_string(items, selector, qualifier);
        }
        let mut sel_ir = self.cg.lower_expr(&self.path, sel)?;
        if case_type == DbCaseKind::Inside {
            return self.lower_case_inside(items, sel_ir, qualifier);
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
            if !qualifier.is_none() {
                return Ok(vec![
                    selector_init,
                    IrStmt::Case {
                        sel: sel_ir,
                        kind: IrCaseKind::Real,
                        items: ir_items,
                        check: qualifier,
                    },
                ]);
            }
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
                    check: IrUniquePriorityCheck::None,
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
            check: qualifier,
        }])
    }

    fn lower_case_inside(
        &mut self,
        items: &[crate::core::db::CaseItem],
        selector: IrExpr,
        check: IrUniquePriorityCheck,
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
        let mut ir_items = Vec::new();
        let mut default = None;
        for item in items {
            let body = match item.body {
                Some(stmt) => self.lower_stmt(stmt)?,
                None => Vec::new(),
            };
            if item.exprs.is_empty() {
                if !check.is_none() {
                    ir_items.push(IrCaseItem {
                        exprs: Vec::new(),
                        body,
                    });
                    continue;
                }
                if default.replace(body).is_some() {
                    return Err(format!(
                        "case inside has multiple default items in `{}`",
                        self.path
                    ));
                }
                continue;
            }

            let members = self.cg.lower_inside_items(&self.path, &item.exprs)?;
            let condition = IrExpr::new(
                IrExprKind::Inside {
                    value: Box::new(selector_read.clone()),
                    items: members,
                },
                1,
                false,
                None,
            );
            if !check.is_none() {
                ir_items.push(IrCaseItem {
                    exprs: vec![condition],
                    body,
                });
                continue;
            }
            branches.push((condition, body));
        }

        if !check.is_none() {
            return Ok(vec![
                IrStmt::DeclLocal {
                    name: selector_name,
                    width: selector_width,
                    signed: selector_signed,
                    two_state: false,
                    init: Some(Box::new(selector)),
                },
                IrStmt::Case {
                    sel: selector_read,
                    kind: IrCaseKind::Inside,
                    items: ir_items,
                    check,
                },
            ]);
        }

        let mut tail = default;
        for (cond, then_) in branches.into_iter().rev() {
            tail = Some(vec![IrStmt::If {
                cond,
                then_,
                els: tail,
                check: IrUniquePriorityCheck::None,
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

    fn lower_case_inside_string(
        &mut self,
        items: &[crate::core::db::CaseItem],
        selector: IrStringExpr,
        check: IrUniquePriorityCheck,
    ) -> Result<Vec<IrStmt>, String> {
        let selector_name = self.new_label("cis");
        let selector_read = IrStringExpr::LocalRead(selector_name.clone());
        let mut branches = Vec::new();
        let mut ir_items = Vec::new();
        let mut default = None;
        for item in items {
            let body = match item.body {
                Some(stmt) => self.lower_stmt(stmt)?,
                None => Vec::new(),
            };
            if item.exprs.is_empty() {
                if !check.is_none() {
                    ir_items.push(IrCaseItem {
                        exprs: Vec::new(),
                        body,
                    });
                    continue;
                }
                if default.replace(body).is_some() {
                    return Err(format!(
                        "case inside has multiple default items in `{}`",
                        self.path
                    ));
                }
                continue;
            }
            let members = self.cg.lower_inside_string_items(&self.path, &item.exprs)?;
            let condition = object_query(
                IrObjectQuery::StringInside {
                    value: selector_read.clone(),
                    items: members,
                },
                1,
                false,
            );
            if !check.is_none() {
                ir_items.push(IrCaseItem {
                    exprs: vec![condition],
                    body,
                });
                continue;
            }
            branches.push((condition, body));
        }
        // A qualified string `case inside` has no packed selector to compare
        // against, so the runtime qualifier walks the membership predicates as
        // `Inside` case items and evaluates the selector exactly once through
        // the captured string local.
        if !check.is_none() {
            return Ok(vec![
                IrStmt::DeclString {
                    name: selector_name,
                    init: Some(selector),
                },
                IrStmt::Case {
                    sel: loop_index_expr(0),
                    kind: IrCaseKind::Inside,
                    items: ir_items,
                    check,
                },
            ]);
        }
        let mut tail = default;
        for (cond, then_) in branches.into_iter().rev() {
            tail = Some(vec![IrStmt::If {
                cond,
                then_,
                els: tail,
                check: IrUniquePriorityCheck::None,
            }]);
        }
        let mut lowered = vec![IrStmt::DeclString {
            name: selector_name,
            init: Some(selector),
        }];
        lowered.extend(tail.unwrap_or_default());
        Ok(lowered)
    }

    pub(super) fn lower_for(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
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
                init: default_real_local_initializer(info.width),
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

    pub(super) fn lower_foreach(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
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
        let mut declarations = Vec::new();

        // A zero-length foreach list is explicitly a no-op in SystemVerilog;
        // it is useful for generated code and must not accidentally execute
        // the body once.  The owned DB retains the list length, including
        // trailing omitted dimensions, so this check is unambiguous.
        if vars.is_empty() {
            return Ok(Vec::new());
        }

        let local_decl = |info: &ProcLocalInfo| IrStmt::DeclLocal {
            name: info.c_name.clone(),
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            init: default_real_local_initializer(info.width),
        };
        let local_read = |info: &ProcLocalInfo| {
            IrExpr::new(
                IrExprKind::LocalRead(info.c_name.clone()),
                info.width,
                info.signed,
                None,
            )
        };
        let local_lhs = |info: &ProcLocalInfo| IrLhs::WholeRef {
            addr: format!("&{}", info.c_name),
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            shortreal: false,
        };

        if let Some(array_info) = self.cg.array_globals.get(&array).cloned() {
            // A foreach list may name only a prefix of an unpacked array's
            // dimensions.  Omitted entries in that prefix skip just that
            // dimension; dimensions not present in the list are left for the
            // body to index explicitly, as required by §12.7.3.
            if vars.len() > array_info.dims.len() {
                return Err(format!(
                    "`foreach` over {}-dimensional array `{}` in `{}` has too many dimensions",
                    array_info.dims.len(),
                    self.cg.node(array).name,
                    self.path
                ));
            }

            // An omitted iterator does not synthesize a loop. If every source
            // slot is omitted, there are no traversed dimensions and the body
            // must not run.
            if vars.iter().all(Option::is_none) {
                return Ok(Vec::new());
            }

            let mut locals = Vec::with_capacity(vars.len());
            for variable in &vars {
                let info = variable
                    .map(|variable| self.cg.collect_loop_var(&self.path, variable))
                    .transpose()?;
                if let Some(info) = &info {
                    if info.width == 0 {
                        return Err(format!(
                            "`foreach` iterator `{}` in `{}` must be an integral index",
                            variable
                                .map(|id| self.cg.node(id).name.as_str())
                                .unwrap_or(""),
                            self.path
                        ));
                    }
                    declarations.push(local_decl(info));
                }
                locals.push(info);
            }

            let (source_body, brk) = self.lower_loop_body(body)?;
            let mut nested = source_body;
            for (local, (left, right)) in locals
                .iter()
                .zip(array_info.dims.iter().take(vars.len()))
                .rev()
            {
                let Some(local) = local else {
                    // An omitted dimension is not traversed and does not
                    // consume or synthesize an iterator local.
                    continue;
                };
                let read = || local_read(local);
                let init = IrStmt::Assign {
                    lhs: local_lhs(local),
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
                    lhs: local_lhs(local),
                    rhs: IrExpr::resize_to(next, local.width, local.signed),
                    nba: false,
                };
                nested.push(IrStmt::If {
                    cond: at_endpoint,
                    then_: vec![IrStmt::Goto(done.clone())],
                    els: None,
                    check: IrUniquePriorityCheck::None,
                });
                nested.push(incr);
                nested = vec![
                    IrStmt::Block(vec![init, IrStmt::Forever { body: nested }]),
                    IrStmt::Label(done),
                ];
            }
            nested.extend(brk);
            declarations.extend(nested);
            return Ok(vec![IrStmt::Block(declarations)]);
        }

        let Some(container) = self.cg.container_globals.get(&array).cloned() else {
            return Err(format!(
                "`foreach` target `{}` in `{}` is not a supported fixed array or container",
                self.cg.node(array).name,
                self.path
            ));
        };
        if vars.len() != 1 {
            return Err(format!(
                "`foreach` over resizable container `{}` in `{}` supports one dimension",
                self.cg.node(array).name,
                self.path
            ));
        }
        let Some(variable) = vars[0] else {
            // The one supplied dimension is omitted, so the loop has no
            // traversed dimension and its body is not executed.
            return Ok(Vec::new());
        };
        let element = self.cg.model.containers[container.ir].element.clone();
        let kind = self.cg.model.containers[container.ir].kind.clone();
        let is_associative = matches!(&kind, IrContainerKind::Associative { .. });
        if is_associative
            && matches!(
                &kind,
                IrContainerKind::Associative {
                    key: IrAssocKey::Wildcard
                }
            )
        {
            return Err(format!(
                "`foreach` over wildcard associative array `{}` in `{}` has no legal key iterator",
                self.cg.node(array).name,
                self.path
            ));
        }
        if !is_associative {
            let local = self.cg.collect_loop_var(&self.path, variable)?;
            if local.width == 0 {
                return Err(format!(
                    "`foreach` iterator `{}` in `{}` must be an integral key",
                    self.cg.node(variable).name,
                    self.path
                ));
            }
            declarations.push(local_decl(&local));
            let (source_body, brk) = self.lower_loop_body(body)?;
            let read = || local_read(&local);
            if !matches!(
                &kind,
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
            ) {
                return Err(format!(
                    "`foreach` target `{}` in `{}` has an unsupported container kind",
                    self.cg.node(array).name,
                    self.path
                ));
            }
            if matches!(element, IrContainerElement::Container { .. }) {
                return Err(format!(
                    "nested resizable container foreach target `{}` in `{}` is not supported",
                    self.cg.node(array).name,
                    self.path
                ));
            }
            let size = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Size(container.ir))),
                32,
                true,
                None,
            );
            let cond = common_bin_expr(IrBinOp::Lt, read(), size);
            let next = common_bin_expr(IrBinOp::Add, read(), loop_index_expr(1));
            let init = IrStmt::Assign {
                lhs: local_lhs(&local),
                rhs: IrExpr::resize_to(loop_index_expr(0), local.width, local.signed),
                nba: false,
            };
            let incr = IrStmt::Assign {
                lhs: local_lhs(&local),
                rhs: IrExpr::resize_to(next, local.width, local.signed),
                nba: false,
            };
            declarations.push(IrStmt::For {
                init: vec![init],
                cond,
                incr: vec![incr],
                body: source_body,
            });
            declarations.extend(brk);
            return Ok(vec![IrStmt::Block(declarations)]);
        }

        // Associative arrays are ordered by key.  The traversal expressions
        // update the iterator in place and return whether a key was found;
        // the first/next choice is guarded by a one-bit state local so first
        // is called once and mutations made by the body are observed by next.
        enum AssocKeyStorage {
            Integral(ProcLocalInfo, u32, bool, bool),
            StringObject(usize),
            StringLocal(String),
        }
        let key_storage = match &kind {
            IrContainerKind::Associative {
                key:
                    IrAssocKey::Integral {
                        width,
                        signed,
                        two_state,
                    },
            } => {
                let local = self.cg.collect_loop_var(&self.path, variable)?;
                if local.width == 0 {
                    return Err(format!(
                        "`foreach` iterator `{}` in `{}` must be an integral key",
                        self.cg.node(variable).name,
                        self.path
                    ));
                }
                declarations.push(local_decl(&local));
                AssocKeyStorage::Integral(local, *width, *signed, *two_state)
            }
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => {
                if let Some(object) = self.cg.object_of(&self.path, variable) {
                    if self.cg.model.objects[object].ty != crate::sim::ir::IrObjectType::String {
                        return Err(format!(
                            "`foreach` iterator `{}` in `{}` is not a string key",
                            self.cg.node(variable).name,
                            self.path
                        ));
                    }
                    AssocKeyStorage::StringObject(object)
                } else {
                    let name = if let Some(name) =
                        self.cg.proc_string_local_name(variable).map(str::to_owned)
                    {
                        name
                    } else {
                        let name = self.cg.collect_loop_string_var(&self.path, variable)?;
                        declarations.push(IrStmt::DeclString {
                            name: name.clone(),
                            init: None,
                        });
                        name
                    };
                    AssocKeyStorage::StringLocal(name)
                }
            }
            _ => unreachable!("associative kind checked above"),
        };
        let (source_body, brk) = self.lower_loop_body(body)?;
        let first_name = format!("_fe_first_{}", h.0);
        let first_info = ProcLocalInfo {
            c_name: first_name.clone(),
            width: 1,
            signed: false,
            two_state: true,
            static_signal: None,
        };
        declarations.push(IrStmt::DeclLocal {
            name: first_name.clone(),
            width: 1,
            signed: false,
            two_state: true,
            init: Some(Box::new(loop_index_expr(1))),
        });
        let traverse = |direction: IrAssocTraversal| {
            let operation = match &key_storage {
                AssocKeyStorage::Integral(local, key_width, key_signed, key_two_state) => {
                    IrContainerExpr::AssocTraverse {
                        container: container.ir,
                        direction,
                        key_address: format!("&{}", local.c_name),
                        key_signal: None,
                        key_width: *key_width,
                        key_signed: *key_signed,
                        key_two_state: *key_two_state,
                    }
                }
                AssocKeyStorage::StringObject(key_object) => IrContainerExpr::AssocTraverseString {
                    container: container.ir,
                    direction,
                    key_object: *key_object,
                },
                AssocKeyStorage::StringLocal(key_name) => {
                    IrContainerExpr::AssocTraverseStringLocal {
                        container: container.ir,
                        direction,
                        key_name: key_name.clone(),
                    }
                }
            };
            IrExpr::new(IrExprKind::Container(Box::new(operation)), 32, true, None)
        };
        let done = self.new_label("fe");
        let first_read = IrExpr::new(
            IrExprKind::LocalRead(first_info.c_name.clone()),
            first_info.width,
            first_info.signed,
            None,
        );
        let clear_first = IrStmt::Assign {
            lhs: IrLhs::WholeRef {
                addr: format!("&{}", first_info.c_name),
                width: first_info.width,
                signed: first_info.signed,
                two_state: first_info.two_state,
                shortreal: false,
            },
            rhs: loop_index_expr(0),
            nba: false,
        };
        let stop_if_missing = |condition: IrExpr| IrStmt::If {
            cond: condition,
            then_: Vec::new(),
            els: Some(vec![IrStmt::Goto(done.clone())]),
            check: IrUniquePriorityCheck::None,
        };
        let select_next = stop_if_missing(traverse(IrAssocTraversal::Next));
        let select_first = stop_if_missing(traverse(IrAssocTraversal::First));
        let select = IrStmt::If {
            cond: first_read,
            then_: vec![clear_first, select_first],
            els: Some(vec![select_next]),
            check: IrUniquePriorityCheck::None,
        };
        declarations.push(IrStmt::Forever {
            body: {
                let mut body = vec![select];
                body.extend(source_body);
                body
            },
        });
        declarations.push(IrStmt::Label(done));
        declarations.extend(brk);
        Ok(vec![IrStmt::Block(declarations)])
    }
}
