//! Calls.

use super::*;

impl EmitCtx<'_, '_> {
    /// Lower a `task_call` statement (or a function call used as a statement).
    /// Delay-bearing tasks are inlined at the call site; delay-free tasks (and
    /// functions) become IR calls with caller-side temps for output formals.
    pub(super) fn lower_task_call(
        &mut self,
        h: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<IrStmt, String> {
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
            if is_task && self.cg.task_has_wait(ft, callee_inst) {
                return Err(format!(
                    "timing-bearing virtual-interface task `{name}` is not supported in `{}`",
                    self.path
                ));
            }
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
            self.cg.class_method_receiver(h)?
        };
        let virtual_call = if let Some((descriptor, method, _, _, receiver)) = virtual_call_info {
            Some(crate::sim::ir::IrVirtualCall {
                interface: descriptor,
                method,
                receiver: self.cg.lower_chandle(&self.path, receiver)?,
            })
        } else {
            None
        };
        let call_receiver = LoweredCallReceiver {
            class: receiver,
            virtual_interface: virtual_call,
        };
        let has_event_formal = bound.iter().any(|argument| argument.is_event);
        if is_task && self.cg.is_class_method_call(h) && self.cg.task_has_wait(ft, callee_inst) {
            return Err(format!(
                "timing-bearing class task `{name}` is not supported in `{}`",
                self.path
            ));
        }
        if has_event_formal || (is_task && self.cg.task_requires_event_inline(ft, callee_inst)) {
            return self.lower_task_inline(
                ft,
                callee_inst,
                h,
                &formals,
                &bound,
                call_receiver.class,
            );
        }
        let can_be_disabled = is_task
            && (self.cg.task_has_disable(ft, callee_inst) || self.cg.task_is_disable_target(ft));
        if can_be_disabled && self.cg.task_has_wait(ft, callee_inst) {
            // Timed cancellation must unwind the callee before caller-side
            // copy-out. Delay-free calls use their native activation scope,
            // including recursive calls. Keep the timed path inline until task returns
            // carry an explicit cancellation result in the C ABI. The
            // declaration-level target check covers callers that disable a
            // task externally rather than from inside the task body.
            self.lower_task_inline(ft, callee_inst, h, &formals, &bound, call_receiver.class)
        } else {
            let fidx = self
                .cg
                .func_meta
                .get(&ft)
                .map(|m| m.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            let call =
                self.lower_call_stmts(fidx, callee_inst, h, &formals, &bound, call_receiver)?;
            if can_be_disabled && !self.cg.class_nodes.contains_key(&callee_inst) {
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
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
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
                        self.cg
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
                    arg_irs[idx] = Some(read_ir);
                }
                continue;
            }
            if !*is_out {
                continue;
            }
            if bound[idx].string {
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
                out_args.push(IrCallArg::OutAddr(format!("&{}", storage.global)));
                continue;
            }
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
                if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.cg.lower_string(&self.path, bound[idx].expr)?,
                    ));
                } else {
                    let ir =
                        self.cg
                            .lower_bound_arg(&self.path, formals, bound, idx, &mut arg_irs)?;
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
            receiver: call_receiver.class,
            virtual_dispatch: self.cg.class_method_virtual_dispatch(h),
            virtual_call: call_receiver.virtual_interface,
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
        class_receiver: Option<IrChandleExpr>,
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
        let mut process_locals: HashMap<NodeId, String> = HashMap::new();
        let mut local_seq = 0usize;
        let prefix = format!("_i{}", h.0);
        self.cg.collect_func_locals(
            body,
            callee_inst,
            &mut locals,
            &mut chandle_locals,
            &mut process_locals,
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
        let mut process_read = HashMap::new();
        let mut process_write = HashMap::new();
        let mut string_read = HashMap::new();
        let mut string_write = HashMap::new();
        let mut string_addr = HashMap::new();
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
                // Output event formals have no input value, but their actual
                // still supplies the destination that receives the handle on
                // return. Resolve it independently from the input capture so
                // `task(..., output event e)` can rebind a caller handle.
                let actual = (!self.cg.is_null_event_expression(b.expr))
                    .then(|| self.cg.event_target_of(b.expr))
                    .flatten();
                let event = if output_only {
                    IrEventRef::Null
                } else if let Some(target) = actual.as_ref() {
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
            if matches!(self.cg.kind(*io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind))
            {
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
                    NodeKind::FuncArg {
                        const_ref: true,
                        ..
                    }
                );
                if is_ref {
                    if !const_ref {
                        self.cg.ensure_string_actual_writable(&self.path, b.expr)?;
                    }
                    let address = self.cg.lower_string_actual_address(&self.path, b.expr)?;
                    let target = address.trim_start_matches('&').to_owned();
                    string_read.insert(*io, self.cg.lower_string(&self.path, b.expr)?);
                    string_addr.insert(*io, target.clone());
                    if !const_ref {
                        string_write.insert(*io, target);
                    }
                } else if let Some(object) = (!automatic)
                    .then(|| {
                        self.cg
                            .static_string_formals
                            .get(&(callee_inst, *io))
                            .copied()
                    })
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
                        self.cg.ensure_string_actual_writable(&self.path, b.expr)?;
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
                    self.cg.ensure_string_actual_writable(&self.path, b.expr)?;
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
                let (width, signed, two_state, actual_const) =
                    self.cg.ref_lhs_type(&actual_lhs).ok_or_else(|| {
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
                arg_ir.insert(*io, read_ir.clone());
                arg_dependencies.insert(*io, self.cg.collect_read_signals(&self.path, b.expr)?);
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
                    let value =
                        self.cg
                            .lower_bound_arg(&self.path, formals, bound, idx, &mut arg_irs)?;
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
                let (actual_lhs, actual_read, selector_inits) =
                    self.cg
                        .lower_call_actual(&self.path, b.expr, &format!("{}_{}", h.0, idx))?;
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
                } else {
                    None
                };
                before.push(IrStmt::DeclLocal {
                    name: cname.clone(),
                    width: b.width,
                    signed: b.signed,
                    two_state: b.two_state,
                    init: init
                        .map(Box::new)
                        .or_else(|| default_real_local_initializer(b.width)),
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
                    arg_dependencies.insert(*io, self.cg.collect_read_signals(&self.path, b.expr)?);
                }
                after.push(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.cg.model, &actual_lhs, read_ir.clone()),
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
            } else {
                let ir = self
                    .cg
                    .lower_bound_arg(&self.path, formals, bound, idx, &mut arg_irs)?;
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

        for (local, cname) in &process_locals {
            process_read.insert(
                *local,
                crate::sim::ir::IrProcessExpr::LocalRead(cname.clone()),
            );
            process_write.insert(*local, ProcessTarget::Local(cname.clone()));
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
                .ok_or_else(|| format!("static task `{tname}` local has no persistent storage"))?;
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
            process_read,
            process_write,
            string_read,
            string_write,
            string_addr,
            locals,
            ret_node: None,
            class_receiver,
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
        let saved_cg = (self.cg.func.take(), self.cg.depth_arg.clone(), self.cg.inst);
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
    pub(super) fn lower_return(&mut self, value: Option<NodeId>) -> Result<IrStmt, String> {
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
        let returns_chandle = self.func.as_ref().is_some_and(|function| {
            function.ret_node.is_some_and(|node| {
                matches!(function.chandle_write.get(&node),
                    Some(ChandleTarget::Local(name)) if name == "_ret")
            })
        });
        if returns_chandle {
            return match value {
                Some(value) => Ok(IrStmt::Block(vec![
                    IrStmt::Object(IrObjectStmt::ChandleAssignLocal(
                        "_ret".to_owned(),
                        self.cg.lower_chandle(&self.path, value)?,
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
