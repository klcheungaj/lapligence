//! Calls.

use super::super::collection::CallWriteback;
use super::*;

impl EmitCtx<'_, '_> {
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
        let call_receiver = LoweredCallReceiver {
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
        let method = call_receiver.class.is_some() || virtual_call_info.is_some();
        self.cg
            .reject_method_record_refs(&self.path, ft, &records, method)?;
        let mut specialized = None;
        if !shape.static_refs.is_empty() {
            if let Some(statics) =
                self.static_ref_actuals(&formals, &bound, &shape.static_refs, &records)?
            {
                specialized =
                    Some(
                        self.cg
                            .task_specialization(ft, callee_inst, statics, records.clone())?,
                    );
            }
        }
        if specialized.is_none() && !records.is_empty() {
            let template = self
                .cg
                .func_meta
                .get(&ft)
                .map(|meta| meta.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            specialized = Some(self.cg.record_ref_callee(
                &self.path,
                ft,
                callee_inst,
                &formals,
                &bound,
                template,
                method,
            )?);
        }
        if specialized.is_none() && self.cg.waits_on_record_ref(ft, callee_inst) {
            // The body reads the record's leaves, which only a module,
            // static or procedural-block actual binds (SIM-008).
            let node = self.cg.node(h);
            return Err(format!(
                "task `{name}` waits on a native record `ref` formal, so each of its record actuals must be a module, static or procedural-block record (SIM-009) at {}:{}:{} in `{}`",
                node.file.as_deref().unwrap_or("<unknown>"),
                node.line,
                node.col,
                self.cg.source_path(&self.path)
            ));
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
        let call = self.lower_call_stmts(fidx, callee_inst, h, &formals, &bound, call_receiver)?;
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
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if self.cg.is_subroutine_container(*io) {
                let argument = self.cg.container_call_argument(
                    &self.path,
                    *io,
                    bound[idx].expr,
                    Some(&mut before),
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
                let event = self.cg.event_ref_of(&target, &self.path)?;
                if matches!(event, IrEventRef::Null | IrEventRef::Handle(_)) {
                    return Err(format!(
                        "event actual for output, inout or ref formal `{}` in `{}` must be an event variable",
                        self.cg.node(*io).name,
                        self.path
                    ));
                }
                out_args.push(IrCallArg::EventAddr(event));
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
                if is_ref || *is_out {
                    let (target, _) = self
                        .cg
                        .lower_chandle_lvalue(&self.path, bound[idx].expr)
                        .map_err(|error| {
                            if is_ref && self.cg.is_container_element(bound[idx].expr) {
                                format!(
                                    "ref actual of handle formal `{}` in `{}` is a container element; element references of handles are not supported (SIM-016)",
                                    self.cg.node(*io).name,
                                    self.cg.source_path(&self.path)
                                )
                            } else {
                                error
                            }
                        })?;
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
            if self.cg.fixed_formal_array(*io).is_some()
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
