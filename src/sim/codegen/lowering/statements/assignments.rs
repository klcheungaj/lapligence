//! Assignments.

use super::*;

impl EmitCtx<'_, '_> {

    /// Lower an assignment without intra-assignment delay (`force_blocking`
    /// pins blocking semantics for for-loop init/increment statements).
    pub(super) fn lower_assignment(&mut self, h: NodeId, force_blocking: bool) -> Result<IrStmt, String> {
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

    pub(super) fn lower_assignment_operands(
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
            } else if self.cg.is_null_event_expression(rhs) {
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
        let mut clocking_targets = Vec::new();
        let all_clocking = self.cg.clocking_lhs_targets(lhs, &mut clocking_targets);
        if !clocking_targets.is_empty() {
            if !all_clocking {
                return Err(format!(
                    "clocking output/inout concatenations cannot mix ordinary targets in `{}`",
                    self.path
                ));
            }
            if blocking {
                return Err(format!(
                    "clocking output/inout member drives in `{}` require nonblocking `<=`",
                    self.path
                ));
            }
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to a clocking output/inout in `{}` is not supported",
                    self.path
                ));
            }
            clocking_targets.sort_unstable_by_key(|target| target.0);
            clocking_targets.dedup();
            for target in &clocking_targets {
                let Some(var) = self.cg.db.clocking_var(*target) else {
                    return Err(format!(
                        "clocking member `{}` has no owned declaration in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    ));
                };
                if matches!(var.direction, DbDirection::Input) {
                    return Err(format!(
                        "clocking input member `{}` is read-only in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    ));
                }
            }
            let lh = self.cg.lower_lhs(&self.path, lhs)?;
            let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
            let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
            let ticks = self
                .cg
                .clocking_output_delay(clocking_targets[0], &self.path)?;
            let drive_specs = self.lower_clocking_drive_specs(clocking_targets[0])?;
            for target in clocking_targets.iter().skip(1) {
                if self.lower_clocking_drive_specs(*target)? != drive_specs {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different clocking events",
                        self.path
                    ));
                }
                let other = self.cg.clocking_output_delay(*target, &self.path)?;
                if other != ticks {
                    return Err(format!(
                        "clocking concatenation in `{}` uses different output skews",
                        self.path
                    ));
                }
            }
            return Ok(IrStmt::ClockingDrive {
                lhs: lh,
                rhs: rhs_ir,
                ticks,
                specs: drive_specs,
            });
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
    pub(super) fn lower_assignment_rhs(
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
        super::super::lower_compound_expr_ir(&self.path, op, lhs, rhs)
    }

    /// Lower a statement-position pre/post increment or decrement.  Since
    /// the operation's value is discarded in statement position, pre and
    /// post forms have the same blocking-write behavior.  Expression-valued
    /// forms require a side-effecting expression IR and remain unsupported.
    pub(super) fn lower_inc_dec(&mut self, op: Operation, operands: &[NodeId]) -> Result<IrStmt, String> {
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
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { .. }) {
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
    pub(super) fn lower_delayed_assignment(
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
            self.cg.ensure_string_actual_writable(&self.path, lhs)?;
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

    /// Lower event/repeat intra-assignment timing. A blocking assignment
    /// evaluates its RHS before registering the wait and evaluates its LHS
    /// when the wait completes. An NBA captures both RHS and destination
    /// selectors at issue time and registers an independent runtime action.
    pub(super) fn lower_event_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        timing: &IntraControl,
    ) -> Result<Vec<IrStmt>, String> {
        if self.in_final {
            return Err(format!(
                "intra-assignment event/repeat control inside a final block in `{}` is not allowed",
                self.path
            ));
        }
        if self.func.is_some() && self.inline.is_none() {
            return Err(format!(
                "event/repeat intra-assignment timing inside a function/task body in `{}` is not supported \
                 (delay-bearing tasks are inlined at their call sites)",
                self.path
            ));
        }
        let (lhs, rhs, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => {
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
                (lhs, rhs, *op)
            }
            _ => unreachable!("non-assignment passed to lower_event_assignment"),
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound event/repeat intra-assignment timing in `{}` is not supported",
                self.path
            ));
        }
        if self.cg.is_string_expr(&self.path, lhs) {
            return Err(format!(
                "event/repeat intra-assignment timing for string storage in `{}` is not supported",
                self.path
            ));
        }
        let (specs, repeat) = self.lower_intra_event_timing(timing)?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);

        if blocking {
            let tmp = format!("_event_rhs_{}", h.0);
            let (w, s) = (rhs_ir.width, rhs_ir.signed);
            let wait = IrStmt::WaitEvents { specs };
            let wait = if let Some(count) = repeat {
                IrStmt::Repeat {
                    count,
                    body: vec![wait],
                }
            } else {
                wait
            };
            self.saw_wait = true;
            return Ok(vec![IrStmt::Block(vec![
                IrStmt::DeclLocal {
                    name: tmp.clone(),
                    width: w,
                    signed: s,
                    two_state: false,
                    init: Some(Box::new(rhs_ir)),
                },
                wait,
                IrStmt::Assign {
                    lhs: lh,
                    rhs: IrExpr::new(IrExprKind::LocalRead(tmp), w, s, None),
                    nba: false,
                },
            ])]);
        }

        if self.cg.proc_local_target(lhs).is_some()
            || self.cg.subroutine_auto_target(lhs)
            || matches!(lh, IrLhs::WholeRef { .. } | IrLhs::Ref { .. })
        {
            return Err(
                "nonblocking event/repeat intra-assignment timing requires persistent target storage"
                    .to_owned(),
            );
        }
        let frame = self.cg.new_frame_id()?;
        let mut captures = Vec::new();
        let rhs_capture = self.capture_event_assignment_expr(frame, &mut captures, rhs_ir);
        let lhs_capture = self.capture_event_assignment_lhs(frame, &mut captures, lh)?;
        let action = self.cg.new_fn_name(&self.path, "event_assign");
        self.pre_fns.push(crate::sim::ir::IrPreFn::EventAssign {
            c_name: action.clone(),
            frame,
            captures: captures.clone(),
            lhs: lhs_capture.clone(),
            rhs: rhs_capture.clone(),
        });
        Ok(vec![IrStmt::NonblockingEventAssignWhen {
            lhs: lhs_capture,
            rhs: rhs_capture,
            specs,
            repeat,
            action,
            frame,
            captures,
        }])
    }

    #[allow(clippy::type_complexity)]
    fn lower_intra_event_timing(
        &mut self,
        timing: &IntraControl,
    ) -> Result<(Vec<(IrWaitSrc, IrEdge)>, Option<IrExpr>), String> {
        match timing {
            IntraControl::Event {
                specs, implicit, ..
            } => {
                if *implicit || specs.is_empty() {
                    return Err(format!(
                        "intra-assignment event control in `{}` must contain an explicit event",
                        self.path
                    ));
                }
                Ok((self.lower_event_specs(specs)?, None))
            }
            IntraControl::Repeat { count, event, .. } => {
                let count = self.cg.lower_expr(&self.path, *count)?;
                if count.is_real() {
                    return Err(format!(
                        "real-valued repeat event count in `{}` is not supported",
                        self.path
                    ));
                }
                let (specs, nested) = self.lower_intra_event_timing(event)?;
                if nested.is_some() {
                    return Err(format!(
                        "nested repeat event controls in `{}` are not supported",
                        self.path
                    ));
                }
                Ok((specs, Some(count)))
            }
            IntraControl::Delay(_) => Err(format!(
                "delay timing reached event-assignment lowering in `{}`",
                self.path
            )),
            IntraControl::Cycle { control, .. } => {
                let node = self.cg.node(*control);
                Err(format!(
                    "cycle delay cannot be nested in an event assignment at {}:{}:{} in `{}`",
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.path
                ))
            }
            IntraControl::Unsupported { control } => {
                let node = self.cg.node(*control);
                Err(format!(
                    "unsupported intra-assignment timing at {}:{}:{} in `{}`",
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col,
                    self.path
                ))
            }
        }
    }

    fn capture_event_assignment_expr(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        expression: IrExpr,
    ) -> IrExpr {
        let slot = captures.len() as u32;
        let storage = StorageRef::new(
            frame,
            slot,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(if expression.is_real() {
            StorageKind::Real
        } else {
            StorageKind::Packed
        });
        let local = format!("_fc{}_{}", frame.index(), slot);
        captures.push(IrCapture::new(storage, expression.clone()));
        IrExpr::new(
            IrExprKind::LocalRead(local),
            expression.width,
            expression.signed,
            None,
        )
    }

    fn capture_event_assignment_lhs(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        lhs: IrLhs,
    ) -> Result<IrLhs, String> {
        Ok(match lhs {
            IrLhs::Bit(index, select, two_state) => IrLhs::Bit(
                index,
                self.capture_event_assignment_expr(frame, captures, select),
                two_state,
            ),
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    index,
                    self.capture_event_assignment_expr(frame, captures, base),
                    self.capture_event_assignment_expr(frame, captures, width),
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => IrLhs::ArrayElem {
                arr,
                indices: indices
                    .into_iter()
                    .map(|index| self.capture_event_assignment_expr(frame, captures, index))
                    .collect(),
                elem_sel: match elem_sel {
                    IrElemSel::Whole => IrElemSel::Whole,
                    IrElemSel::Part(left, right) => IrElemSel::Part(left, right),
                    IrElemSel::Bit(index) => IrElemSel::Bit(Box::new(
                        self.capture_event_assignment_expr(frame, captures, *index),
                    )),
                    IrElemSel::Indexed {
                        base,
                        width,
                        negative,
                    } => IrElemSel::Indexed {
                        base: Box::new(self.capture_event_assignment_expr(frame, captures, *base)),
                        width,
                        negative,
                    },
                },
            },
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, width)| {
                        self.capture_event_assignment_lhs(frame, captures, part)
                            .map(|part| (part, width))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                width,
                slice,
                direction,
            },
            other => other,
        })
    }

    /// Capture selectors of a cycle-delayed clocking drive before its event
    /// wait. Unlike an ordinary blocking event assignment, the clocking drive
    /// uses an NBA-style update after the wait, so dynamic indices must retain
    /// their issue-time values without needing a detached callback frame.
    pub(super) fn capture_cycle_lhs(&self, h: NodeId, lhs: IrLhs) -> (Vec<IrStmt>, IrLhs) {
        fn selector(h: NodeId, slots: &mut Vec<IrStmt>, expr: IrExpr) -> IrExpr {
            let name = format!("_cycle_sel_{}_{}", h.0, slots.len());
            slots.push(IrStmt::DeclLocal {
                name: name.clone(),
                width: expr.width,
                signed: expr.signed,
                two_state: false,
                init: Some(Box::new(expr.clone())),
            });
            IrExpr::new(IrExprKind::LocalRead(name), expr.width, expr.signed, None)
        }

        fn target(h: NodeId, slots: &mut Vec<IrStmt>, lhs: IrLhs) -> IrLhs {
            match lhs {
                IrLhs::Bit(index, select, two_state) => {
                    IrLhs::Bit(index, selector(h, slots, select), two_state)
                }
                IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                    IrLhs::IdxPart(
                        index,
                        selector(h, slots, base),
                        selector(h, slots, width),
                        selected_width,
                        negative,
                        two_state,
                    )
                }
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel,
                } => IrLhs::ArrayElem {
                    arr,
                    indices: indices
                        .into_iter()
                        .map(|index| selector(h, slots, index))
                        .collect(),
                    elem_sel: match elem_sel {
                        IrElemSel::Whole => IrElemSel::Whole,
                        IrElemSel::Part(left, right) => IrElemSel::Part(left, right),
                        IrElemSel::Bit(index) => {
                            IrElemSel::Bit(Box::new(selector(h, slots, *index)))
                        }
                        IrElemSel::Indexed {
                            base,
                            width,
                            negative,
                        } => IrElemSel::Indexed {
                            base: Box::new(selector(h, slots, *base)),
                            width,
                            negative,
                        },
                    },
                },
                IrLhs::Stream {
                    parts,
                    width,
                    slice,
                    direction,
                } => IrLhs::Stream {
                    parts: parts
                        .into_iter()
                        .map(|(part, width)| (target(h, slots, part), width))
                        .collect(),
                    width,
                    slice,
                    direction,
                },
                other => other,
            }
        }

        let mut slots = Vec::new();
        let lhs = target(h, &mut slots, lhs);
        (slots, lhs)
    }
}
