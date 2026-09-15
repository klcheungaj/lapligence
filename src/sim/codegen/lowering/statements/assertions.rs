//! Assertions.

use super::*;

impl<'c, 'a> EmitCtx<'c, 'a> {
    pub(super) fn lower_immediate_assertion(
        &mut self,
        h: NodeId,
        assertion: &StmtKind,
    ) -> Result<Vec<IrStmt>, String> {
        let StmtKind::ImmediateAssertion {
            kind,
            cond,
            if_true,
            if_false,
            label,
            deferred,
            is_final,
        } = assertion
        else {
            unreachable!("immediate assertion lowering received another statement kind")
        };
        if *is_final {
            return Err(format!(
                "final immediate assertions are not supported at {}",
                self.finish_location(h)
            ));
        }
        let condition = self.cg.lower_boolean_expr(&self.path, *cond)?;
        let kind = match *kind {
            ImmediateAssertionKind::Assert => IrImmediateAssertionKind::Assert,
            ImmediateAssertionKind::Assume => IrImmediateAssertionKind::Assume,
            ImmediateAssertionKind::Cover => IrImmediateAssertionKind::Cover,
        };
        if *deferred {
            if self.in_final {
                return Err(format!(
                    "deferred immediate assertions inside a final block are not supported at {}",
                    self.finish_location(h)
                ));
            }
            // A deferred action outlives the statement activation. Ordinary
            // process storage is safe only after its by-value arguments have
            // been copied into the action frame; function/task contexts have
            // no stable callback depth or activation lifetime here.
            if self.func.is_some() || self.inline.is_some() {
                return Err(format!(
                    "deferred immediate assertions inside a function or task are not supported at {}",
                    self.finish_location(h)
                ));
            }
            let if_true = (*if_true)
                .map(|statement| self.lower_deferred_assertion_action(statement))
                .transpose()?;
            let if_false = (*if_false)
                .map(|statement| self.lower_deferred_assertion_action(statement))
                .transpose()?;
            return Ok(vec![IrStmt::DeferredImmediateAssertion {
                kind,
                condition,
                if_true,
                if_false,
                label: label.clone(),
                location: self.finish_location(h),
                scope: self.path.clone(),
                identity: h.index() as u64,
            }]);
        }
        let if_true = (*if_true)
            .map(|statement| self.lower_stmt(statement))
            .transpose()?;
        let if_false = (*if_false)
            .map(|statement| self.lower_stmt(statement))
            .transpose()?;
        Ok(vec![IrStmt::ImmediateAssertion {
            kind,
            condition,
            if_true,
            if_false,
            label: label.clone(),
            location: self.finish_location(h),
            identity: h.index() as u64,
        }])
    }

    /// Lower one deferred immediate-assertion action. Slang has already
    /// restricted the source action to a single subroutine/system-task call;
    /// retain that shape in the IR and reject any lowering that would smuggle
    /// in a block, timing control, output copy-out, or an unowned object.
    fn lower_deferred_assertion_action(
        &mut self,
        statement: NodeId,
    ) -> Result<IrDeferredAction, String> {
        self.validate_deferred_action_refs(statement)?;
        let pre_fn_start = self.pre_fns.len();
        let body = match self.lower_stmt(statement) {
            Ok(body) => body,
            Err(error) => {
                self.pre_fns.truncate(pre_fn_start);
                return Err(error);
            }
        };
        let Some(body_stmt) = (match body.as_slice() {
            [] => None,
            [stmt] => Some(stmt.clone()),
            _ => {
                self.pre_fns.truncate(pre_fn_start);
                return Err(format!(
                    "deferred immediate assertion action at {} must be one subroutine call",
                    self.finish_location(statement)
                ));
            }
        }) else {
            // An empty action is legal after the frontend's statement-or-null
            // normalization. Keep it as a callback so report selection and
            // queue coalescing still have one uniform path.
            let frame = self.cg.new_frame_id()?;
            let action = IrDeferredAction::new(
                self.cg.new_fn_name(&self.path, "assert_deferred"),
                frame,
                Vec::new(),
            );
            self.pre_fns
                .push(crate::sim::ir::IrPreFn::DeferredAssertion {
                    c_name: action.c_name().to_owned(),
                    frame: action.frame(),
                    captures: Vec::new(),
                    body: Vec::new(),
                });
            return Ok(action);
        };

        let frame = self.cg.new_frame_id()?;
        let mut captures = Vec::new();
        let body_stmt = match self.capture_deferred_assertion_stmt(frame, &mut captures, body_stmt)
        {
            Ok(body) => body,
            Err(error) => {
                self.pre_fns.truncate(pre_fn_start);
                return Err(error);
            }
        };
        let action = IrDeferredAction::new(
            self.cg.new_fn_name(&self.path, "assert_deferred"),
            frame,
            captures,
        );
        self.pre_fns
            .push(crate::sim::ir::IrPreFn::DeferredAssertion {
                c_name: action.c_name().to_owned(),
                frame: action.frame(),
                captures: action.captures().to_vec(),
                body: vec![body_stmt],
            });
        Ok(action)
    }

    /// Reference actuals are evaluated by the callback, so only storage with
    /// a lifetime beyond this process statement may be retained. Value
    /// actuals deliberately do not use this check: they are copied below at
    /// assertion execution time, including automatic locals.
    fn validate_deferred_action_refs(&self, statement: NodeId) -> Result<(), String> {
        let call = match self.cg.kind(statement) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => Some((statement, name.as_str(), *is_task, *callee)),
            NodeKind::Stmt(StmtKind::Begin) => {
                self.cg.node(statement).children.iter().find_map(|child| {
                    match self.cg.kind(*child) {
                        NodeKind::FuncCall {
                            name,
                            is_task,
                            callee,
                            ..
                        } => Some((*child, name.as_str(), *is_task, *callee)),
                        _ => None,
                    }
                })
            }
            _ => None,
        };
        let Some((call_node, name, is_task, callee)) = call else {
            return Ok(());
        };
        let (callee_function, callee_inst) = self
            .cg
            .resolve_callee_env(self.inst, name, is_task, callee)?;
        if is_task && self.cg.task_has_wait(callee_function, callee_inst) {
            return Err(format!(
                "deferred immediate assertion action task `{name}` cannot contain timing controls at {}",
                self.finish_location(statement)
            ));
        }
        let (_, _, formals) = self.cg.func_info(callee_function, callee_inst)?;
        let args = self.cg.node(call_node).children.clone();
        let bound = self.cg.bind_call_args(self.inst, &formals, &args)?;
        for (idx, (formal, _)) in formals.iter().enumerate() {
            let is_ref = matches!(
                self.cg.kind(*formal),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !is_ref {
                continue;
            }
            let actual = bound[idx].expr;
            if let Some(local) = self.cg.nested_proc_local_ref(actual) {
                return Err(format!(
                    "deferred immediate assertion action cannot retain automatic ref argument `{}` at {}",
                    self.cg.node(local).name,
                    self.finish_location(statement)
                ));
            }
            if let Some(local) = self.cg.nested_capture_ref(actual) {
                return Err(format!(
                    "deferred immediate assertion action cannot retain captured ref argument `{}` at {}",
                    self.cg.node(local).name,
                    self.finish_location(statement)
                ));
            }
        }
        Ok(())
    }

    /// Replace action-time by-value expressions with frame reads. Reference
    /// descriptors and control-only actions remain untouched so their storage
    /// lookup occurs when Reactive executes the callback.
    fn capture_deferred_assertion_stmt(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        stmt: IrStmt,
    ) -> Result<IrStmt, String> {
        let mut capture =
            |expression: IrExpr| self.capture_deferred_assertion_expr(frame, captures, expression);
        match stmt {
            IrStmt::Display {
                fmt,
                args,
                newline,
                default_radix,
            } => Ok(IrStmt::Display {
                fmt,
                args: args
                    .into_iter()
                    .map(|(value, real)| (capture(value), real))
                    .collect(),
                newline,
                default_radix,
            }),
            IrStmt::DisplayTyped {
                fmt,
                args,
                scope,
                newline,
                default_radix,
                descriptor,
                time_unit_fs,
            } => {
                let args = args
                    .into_iter()
                    .map(|arg| match arg {
                        crate::sim::ir::IrDisplayArg::Packed(value) => {
                            Ok(crate::sim::ir::IrDisplayArg::Packed(capture(value)))
                        }
                        crate::sim::ir::IrDisplayArg::Real(value) => {
                            Ok(crate::sim::ir::IrDisplayArg::Real(capture(value)))
                        }
                        crate::sim::ir::IrDisplayArg::String(_) => Err(
                            "string value arguments in deferred immediate assertion actions are not supported"
                                .to_owned(),
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::DisplayTyped {
                    fmt,
                    args,
                    scope,
                    newline,
                    default_radix,
                    descriptor: descriptor.map(capture),
                    time_unit_fs,
                })
            }
            IrStmt::Severity {
                level,
                fmt,
                args,
                scope,
                location,
                fatal_finish_number,
            } => {
                if level.is_fatal() {
                    return Err(
                        "$fatal is not supported as a deferred immediate assertion action"
                            .to_owned(),
                    );
                }
                let args = args
                    .into_iter()
                    .map(|arg| match arg {
                        crate::sim::ir::IrDisplayArg::Packed(value) => {
                            Ok(crate::sim::ir::IrDisplayArg::Packed(capture(value)))
                        }
                        crate::sim::ir::IrDisplayArg::Real(value) => {
                            Ok(crate::sim::ir::IrDisplayArg::Real(capture(value)))
                        }
                        crate::sim::ir::IrDisplayArg::String(_) => Err(
                            "string value arguments in deferred immediate assertion actions are not supported"
                                .to_owned(),
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::Severity {
                    level,
                    fmt,
                    args,
                    scope,
                    location,
                    fatal_finish_number,
                })
            }
            IrStmt::FileControl { op, descriptor } => Ok(IrStmt::FileControl {
                op,
                descriptor: descriptor.map(capture),
            }),
            IrStmt::WaveLimit(value) => Ok(IrStmt::WaveLimit(capture(value))),
            IrStmt::Call(mut call) => {
                if !call.temps.is_empty() || !call.copyouts.is_empty() {
                    return Err(
                        "deferred immediate assertion actions cannot use output or inout arguments"
                            .to_owned(),
                    );
                }
                for arg in &mut call.args {
                    match arg {
                        IrCallArg::Val(value) => {
                            *value = capture(value.clone());
                        }
                        IrCallArg::RefAddr { .. } | IrCallArg::StringRefAddr { .. } => {}
                        IrCallArg::StringVal(_)
                        | IrCallArg::ChandleVal(_)
                        | IrCallArg::ChandleAddr(_)
                        | IrCallArg::ChandleRefAddr(_)
                        | IrCallArg::OutAddr(_)
                        | IrCallArg::OutTemp { .. }
                        | IrCallArg::StringOutAddr(_)
                        | IrCallArg::StringOutTemp { .. } => {
                            return Err(
                                "deferred immediate assertion actions support only value and static ref arguments"
                                    .to_owned(),
                            )
                        }
                    }
                }
                Ok(IrStmt::Call(call))
            }
            IrStmt::System(None) => Ok(IrStmt::System(None)),
            IrStmt::Nop => Ok(IrStmt::Nop),
            IrStmt::Finish | IrStmt::FinishControl { .. } | IrStmt::StopControl { .. } => Err(
                "simulation termination/control tasks are not supported as deferred immediate assertion actions"
                    .to_owned(),
            ),
            IrStmt::Block(mut body) if body.len() == 1 => {
                let inner = body
                    .pop()
                    .expect("one-element deferred assertion action block");
                Ok(IrStmt::Block(vec![self.capture_deferred_assertion_stmt(
                    frame, captures, inner,
                )?]))
            }
            _ => Err(
                "deferred immediate assertion action must lower to one supported subroutine call"
                    .to_owned(),
            ),
        }
    }

    fn capture_deferred_assertion_expr(
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
        let local = Codegen::capture_local_name(storage);
        captures.push(IrCapture::new(storage, expression.clone()));
        IrExpr::new(
            IrExprKind::LocalRead(local),
            expression.width,
            expression.signed,
            None,
        )
    }
}
