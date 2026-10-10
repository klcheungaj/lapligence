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
            // A deferred action outlives the statement activation: its
            // by-value arguments are copied into an owned action frame, so
            // procedural, subroutine, class-method and final contexts share
            // one path. Each executing process owns its own report queue.
            let if_true = (*if_true)
                .map(|statement| self.lower_deferred_assertion_action(statement))
                .transpose()?;
            let if_false = (*if_false)
                .map(|statement| self.lower_deferred_assertion_action(statement))
                .transpose()?;
            return Ok(vec![IrStmt::DeferredImmediateAssertion {
                kind,
                condition,
                if_true: if_true.map(Box::new),
                if_false: if_false.map(Box::new),
                label: label.clone(),
                location: self.finish_location(h),
                scope: self.cg.format_scope(&self.path, h),
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
            condition: Box::new(condition),
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
                return Err(format!("{error} at {}", self.finish_location(statement)));
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
            if self.dynamic_ref_actual(actual) {
                return Err(format!(
                    "deferred immediate assertion action cannot pass a dynamic variable to ref argument `{}` at {} (SV 16.4)",
                    self.cg.node(formals[idx].0).name,
                    self.finish_location(statement)
                ));
            }
        }
        Ok(())
    }

    /// Whether a ref actual names dynamically allocated storage: an element
    /// or member reached through a dynamic array, queue or associative array
    /// (SV 16.4: "It shall be an error to pass automatic or dynamic variables
    /// as actuals to a ref or const ref formal"). Only the selected prefix
    /// chain is examined, not index expressions.
    fn dynamic_ref_actual(&self, actual: NodeId) -> bool {
        let mut node = actual;
        loop {
            if self
                .cg
                .query_descriptor(node)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::Container { .. }))
                && node != actual
            {
                return true;
            }
            match self.cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref { .. }) => return false,
                NodeKind::Expr(_) => match self.cg.node(node).children.first() {
                    Some(child) => node = *child,
                    None => return false,
                },
                _ => return false,
            }
        }
    }

    /// Replace action-time by-value expressions with frame reads. Reference
    /// descriptors remain untouched so their storage lookup occurs when
    /// Reactive executes the callback (SV 16.4: by-value actuals use their
    /// values when the assertion is evaluated, ref actuals the Reactive-time
    /// values). A method call's receiver handle is captured with the inputs
    /// (llg decision S36-D3).
    fn capture_deferred_assertion_stmt(
        &self,
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        stmt: IrStmt,
    ) -> Result<IrStmt, String> {
        match stmt {
            IrStmt::Located { origin, statement } => self
                .capture_deferred_assertion_stmt(frame, captures, *statement)
                .map(|statement| statement.with_origin(origin)),
            IrStmt::Display {
                fmt,
                args,
                newline,
                default_radix,
            } => Ok(IrStmt::Display {
                fmt,
                args: args
                    .into_iter()
                    .map(|(value, real)| {
                        (Self::capture_deferred_value(frame, captures, value), real)
                    })
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
            } => Ok(IrStmt::DisplayTyped {
                fmt,
                args: args
                    .into_iter()
                    .map(|arg| Self::capture_deferred_display_arg(frame, captures, arg))
                    .collect(),
                scope,
                newline,
                default_radix,
                descriptor: descriptor
                    .map(|value| Self::capture_deferred_value(frame, captures, value)),
                time_unit_fs,
            }),
            IrStmt::Severity {
                level,
                fmt,
                args,
                scope,
                location,
                fatal_finish_number,
                runtime_failure,
            } => Ok(IrStmt::Severity {
                level,
                fmt,
                args: args
                    .into_iter()
                    .map(|arg| Self::capture_deferred_display_arg(frame, captures, arg))
                    .collect(),
                scope,
                location,
                fatal_finish_number,
                runtime_failure,
            }),
            IrStmt::FileControl { op, descriptor } => Ok(IrStmt::FileControl {
                op,
                descriptor: descriptor.map(|descriptor| {
                    Box::new(Self::capture_deferred_value(frame, captures, *descriptor))
                }),
            }),
            IrStmt::WaveLimit(value) => Ok(IrStmt::WaveLimit(Self::capture_deferred_value(
                frame, captures, value,
            ))),
            IrStmt::Call(mut call) => {
                if !call.temps.is_empty() || !call.copyouts.is_empty() {
                    return Err(
                        "deferred immediate assertion actions cannot use output or inout arguments"
                            .to_owned(),
                    );
                }
                if call.virtual_call.is_some() {
                    return Err(
                        "virtual-interface method calls are not supported as deferred immediate assertion actions"
                            .to_owned(),
                    );
                }
                if let Some(receiver) = call.receiver.take() {
                    call.receiver = Some(Self::capture_deferred_handle(frame, captures, receiver));
                }
                for arg in &mut call.args {
                    match arg {
                        IrCallArg::Val(value) => {
                            *value = Self::capture_deferred_value(frame, captures, value.clone());
                        }
                        IrCallArg::StringVal(value) => {
                            *value = Self::capture_deferred_string(frame, captures, value.clone());
                        }
                        IrCallArg::ChandleVal(value) => {
                            *value = Self::capture_deferred_handle(frame, captures, value.clone());
                        }
                        IrCallArg::RefAddr { .. }
                        | IrCallArg::StringRefAddr { .. }
                        | IrCallArg::ChandleRefAddr(_) => {}
                        IrCallArg::FixedValue(_)
                        | IrCallArg::FixedArray(_)
                        | IrCallArg::RealArray(_)
                        | IrCallArg::RealArrayValues(_)
                        | IrCallArg::NativeValue(_)
                        | IrCallArg::NativeRefBound
                        | IrCallArg::Container(_)
                        | IrCallArg::ContainerValues { .. }
                        | IrCallArg::NativeLeaves { .. }
                        | IrCallArg::NativeCall { .. }
                        | IrCallArg::RealArrayCall { .. }
                        | IrCallArg::EventVal(_)
                        | IrCallArg::EventAddr(_) => {
                            return Err(
                                "unpacked aggregate, container and event arguments of deferred immediate assertion actions are not supported"
                                    .to_owned(),
                            )
                        }
                        IrCallArg::ChandleAddr(_)
                        | IrCallArg::OutAddr(_)
                        | IrCallArg::OutTemp { .. }
                        | IrCallArg::StringOutAddr(_)
                        | IrCallArg::StringOutTemp { .. } => {
                            return Err(
                                "deferred immediate assertion actions cannot use output or inout arguments"
                                    .to_owned(),
                            )
                        }
                    }
                }
                Ok(IrStmt::Call(call))
            }
            IrStmt::System(None) => Ok(IrStmt::System(None)),
            IrStmt::Nop => Ok(IrStmt::Nop),
            // Simulation control tasks are legal single-call actions (SV
            // 16.4); they execute once, in the Reactive region.
            statement @ (IrStmt::Finish
            | IrStmt::FinishControl { .. }
            | IrStmt::StopControl { .. }) => Ok(statement),
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

    fn capture_deferred_display_arg(
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        arg: crate::sim::ir::IrDisplayArg,
    ) -> crate::sim::ir::IrDisplayArg {
        use crate::sim::ir::IrDisplayArg;
        match arg {
            IrDisplayArg::Packed(value) => {
                IrDisplayArg::Packed(Self::capture_deferred_value(frame, captures, value))
            }
            IrDisplayArg::Real(value) => {
                IrDisplayArg::Real(Self::capture_deferred_value(frame, captures, value))
            }
            IrDisplayArg::Strength(value) => {
                IrDisplayArg::Strength(Self::capture_deferred_value(frame, captures, value))
            }
            IrDisplayArg::String(value) => {
                IrDisplayArg::String(Self::capture_deferred_string(frame, captures, value))
            }
            IrDisplayArg::Text(value) => {
                IrDisplayArg::Text(Self::capture_deferred_string(frame, captures, value))
            }
        }
    }

    fn capture_deferred_slot(
        frame: FrameId,
        captures: &[IrCapture],
        kind: StorageKind,
    ) -> StorageRef {
        StorageRef::new(
            frame,
            captures.len() as u32,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(kind)
    }

    fn capture_deferred_value(
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        expression: IrExpr,
    ) -> IrExpr {
        // Literals need no issue-time copy.
        if matches!(expression.kind(), IrExprKind::Const(_)) {
            return expression;
        }
        let storage = Self::capture_deferred_slot(
            frame,
            captures,
            if expression.is_real() {
                StorageKind::Real
            } else {
                StorageKind::Packed
            },
        );
        let local = Codegen::capture_local_name(storage);
        let (width, signed) = (expression.width, expression.signed);
        captures.push(IrCapture::new(storage, expression));
        IrExpr::new(IrExprKind::LocalRead(local), width, signed, None)
    }

    fn capture_deferred_string(
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        value: IrStringExpr,
    ) -> IrStringExpr {
        if matches!(value, IrStringExpr::Literal(_)) {
            return value;
        }
        let storage = Self::capture_deferred_slot(frame, captures, StorageKind::String);
        let local = Codegen::capture_local_name(storage);
        captures.push(IrCapture::new(
            storage,
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringCapture(value))),
                1,
                false,
                None,
            ),
        ));
        IrStringExpr::LocalRead(local)
    }

    fn capture_deferred_handle(
        frame: FrameId,
        captures: &mut Vec<IrCapture>,
        value: IrChandleExpr,
    ) -> IrChandleExpr {
        if matches!(value, IrChandleExpr::Null) {
            return value;
        }
        let storage = Self::capture_deferred_slot(frame, captures, StorageKind::Opaque);
        let local = Codegen::capture_local_name(storage);
        captures.push(IrCapture::new(
            storage,
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(value))),
                1,
                false,
                None,
            ),
        ));
        IrChandleExpr::LocalRead(local)
    }
}
