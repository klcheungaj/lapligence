//! Calls.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn ref_lhs_type(&self, lhs: &IrLhs) -> Option<(u32, bool, bool, bool)> {
        match lhs {
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            } => Some((
                steps.last()?.width,
                *signed,
                *two_state || self.ref_lhs_type(target)?.2,
                self.ref_lhs_type(target)?.3,
            )),
            IrLhs::Whole(index) => match self.model.signal(*index).ty {
                IrType::Packed {
                    width,
                    signed,
                    two_state,
                } => Some((width, signed, two_state, false)),
                IrType::Real { .. } => None,
            },
            IrLhs::WholeRef {
                width,
                signed,
                two_state,
                ..
            } => Some((*width, *signed, *two_state, false)),
            IrLhs::Ref {
                width,
                signed,
                two_state,
                const_ref,
                ..
            } => Some((*width, *signed, *two_state, *const_ref)),
            IrLhs::Bit(_, _, two_state) => Some((1, false, *two_state, false)),
            IrLhs::Part(_, left, right, two_state) => {
                Some((left.abs_diff(*right) as u32 + 1, false, *two_state, false))
            }
            IrLhs::IdxPart(_, _, _, width, _, two_state) => {
                Some((*width, false, *two_state, false))
            }
            IrLhs::ArrayElem { arr, elem_sel, .. } => {
                let array = self.model.arrays.get(*arr)?;
                let (width, signed) = match elem_sel {
                    IrElemSel::Whole => (array.elem_width, array.signed),
                    IrElemSel::Part(left, right) => (left.abs_diff(*right) as u32 + 1, false),
                    IrElemSel::Bit(_) => (1, false),
                    IrElemSel::Indexed { width, .. } => (*width, false),
                    IrElemSel::PackedChain(steps) => {
                        (steps.last().map_or(0, |step| step.width), false)
                    }
                };
                Some((width, signed, array.two_state, false))
            }
            IrLhs::Stream { .. } => None,
        }
    }

    /// Lower a `func_call` expression used as a value: `fn_<callee>(<args>,
    /// <depth>)` with output/inout formals bound to caller-side temps that
    /// are written back into the bound actuals after the call (the backend
    /// wraps those into one GNU statement expression).
    pub(in super::super) fn lower_func_call_expr(
        &mut self,
        scope_path: &str,
        h: NodeId,
        name: &str,
        callee: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        let args = self.call_argument_nodes(h);
        self.lower_func_call_expr_with_args(scope_path, h, name, callee, &args)
    }

    /// Lower a resolved function call with an explicit argument list.  The
    /// class constructor path uses this for Slang's implicit base
    /// `super.new()` call, whose source has no call-expression node of its
    /// own but still needs the callee's default argument binding.
    pub(in super::super) fn lower_func_call_expr_with_args(
        &mut self,
        scope_path: &str,
        h: NodeId,
        name: &str,
        callee: Option<NodeId>,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let virtual_call_info = self.virtual_interface_method_info(h)?;
        let (ft, callee_inst) = if let Some((_, _, ft, callee_inst, _)) = virtual_call_info {
            (ft, callee_inst)
        } else {
            self.resolve_callee_env(self.inst, name, false, callee)?
        };
        let meta = self
            .func_meta
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{name}` has no C name"))?;
        if meta.is_task {
            return Err(format!(
                "task call `{name}` used as an expression in `{scope_path}`"
            ));
        }
        let formals = meta.formals.clone();
        let bound = self.bind_call_args(self.inst, &formals, args)?;
        for (idx, (io, _is_out)) in formals.iter().enumerate() {
            if bound[idx].is_event {
                return Err(format!(
                    "event formal `{}` in function expression `{name}` has no typed value call path",
                    self.node(*io).name
                ));
            }
        }
        // `ret` is `None` for void functions; when the frontend accepts one as
        // a value (for example `out <= vf(4'd2);`), emit the call for its
        // side effects and yields all-X.
        let ret_val = meta.ret;
        let (ret_w, ret_s, _, _) = ret_val.unwrap_or((1, false, false, false));

        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if matches!(
                self.kind(*io),
                NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
            ) {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    let (target, _) = self.lower_chandle_lvalue(scope_path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                }
                continue;
            }
            let is_ref = matches!(
                self.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if is_ref {
                let (const_ref, ref_static) = match self.kind(*io) {
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
                        self.ensure_string_actual_writable(scope_path, bound[idx].expr)?;
                    }
                    out_args.push(IrCallArg::StringRefAddr {
                        addr: self.lower_string_actual_address(scope_path, bound[idx].expr)?,
                        const_ref,
                    });
                } else {
                    out_args.push(self.lower_ref_arg(
                        scope_path,
                        &bound[idx],
                        const_ref,
                        ref_static,
                    )?);
                }
                if !bound[idx].string {
                    let read_ir = self.lower_expr(scope_path, bound[idx].expr)?;
                    arg_irs[idx] = Some(read_ir);
                }
                continue;
            }
            if *is_out {
                if bound[idx].string {
                    self.ensure_string_actual_writable(scope_path, bound[idx].expr)?;
                    let tname = format!("_st{}_{}", h.0, idx);
                    let writeback =
                        self.lower_string_actual_address(scope_path, bound[idx].expr)?;
                    let init = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    )
                    .then(|| self.lower_string(scope_path, bound[idx].expr))
                    .transpose()?;
                    let (storage_addr, storage_read) = self
                        .static_string_formals
                        .get(&(callee_inst, *io))
                        .map(|object| {
                            (
                                Some(format!("&{}", self.model.objects[*object].c_name)),
                                Some(Box::new(IrStringExpr::Read(*object))),
                            )
                        })
                        .unwrap_or((None, None));
                    out_args.push(IrCallArg::StringOutTemp {
                        name: tname,
                        init: init.map(Box::new),
                        writeback,
                        storage_addr,
                        storage_read,
                    });
                    continue;
                }
                let tname = format!("_t{}_{}", h.0, idx);
                let (wb, actual_read, selector_inits) =
                    self.lower_call_actual(scope_path, bound[idx].expr, &format!("{}_{idx}", h.0))?;
                let init_ir = self.lower_call_temp_init_from_expr(*io, &bound[idx], actual_read)?;
                let storage = self.static_formals.get(&(callee_inst, *io)).cloned();
                let (storage_addr, storage_lhs, storage_read) = if let Some(storage) = storage {
                    let lhs = IrLhs::Whole(storage.ir);
                    let read = sig_read_expr_full(&storage);
                    (
                        Some(format!("&{}", storage.global)),
                        Some(Box::new(lhs)),
                        Some(Box::new(read)),
                    )
                } else {
                    (None, None, None)
                };
                // The temp is the correctly-sized value of the formal while
                // the call runs (all-X for outputs, the actual for inouts).
                arg_irs[idx] = Some(IrExpr::new(
                    IrExprKind::LocalRead(tname.clone()),
                    bound[idx].width,
                    bound[idx].signed,
                    None,
                ));
                out_args.push(IrCallArg::OutTemp {
                    name: tname,
                    init: init_ir.map(Box::new),
                    writeback: Box::new(wb),
                    storage_addr,
                    storage_lhs,
                    storage_read,
                    selector_inits,
                });
            }
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let is_ref = matches!(
                self.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !*is_out
                && !is_ref
                && matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind))
            {
                in_args.push(IrCallArg::ChandleVal(
                    self.lower_chandle(scope_path, bound[idx].expr)?,
                ));
            } else if !*is_out && !is_ref {
                if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.lower_string(scope_path, bound[idx].expr)?,
                    ));
                    continue;
                }
                let ir = self.lower_bound_arg(scope_path, &formals, &bound, idx, &mut arg_irs)?;
                arg_irs[idx] = Some(IrExpr::new(
                    IrExprKind::LocalRead(crate::sim::ir::call_argument_name(idx)),
                    ir.width,
                    ir.signed,
                    None,
                ));
                in_args.push(IrCallArg::Val(ir));
            }
        }
        out_args.extend(in_args);
        // A class method written without an explicit receiver inside another
        // class method is represented as a plain function call by Slang. Bind
        // that call to the current `this` (or the object under construction).
        let (receiver, virtual_call) =
            if let Some((descriptor, method, _, _, receiver)) = virtual_call_info {
                (
                    None,
                    Some(crate::sim::ir::IrVirtualCall {
                        interface: descriptor,
                        method,
                        receiver: self.lower_chandle(scope_path, receiver)?,
                    }),
                )
            } else if self.model.funcs[meta.ir].receiver_class.is_some()
                && matches!(self.kind(h), NodeKind::FuncCall { .. })
            {
                (
                    self.class_init_receiver.clone().or_else(|| {
                        self.func
                            .as_ref()
                            .and_then(|function| function.class_receiver.clone())
                    }),
                    None,
                )
            } else {
                (None, None)
            };
        let is_class_constructor =
            self.class_method_owner(ft).is_some() && self.node(ft).name == "new";
        if ret_val.is_none() && !is_class_constructor && !self.lowering_assertion_match_item {
            self.warnings.push(format!(
                "void function `{name}` used as a value in `{scope_path}`; result is X"
            ));
        }
        let depth = parse_depth(&self.depth_arg);
        Ok(IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr {
                f: meta.ir,
                args: out_args,
                depth,
                receiver,
                virtual_dispatch: false,
                virtual_call,
                void_x: ret_val.is_none(),
            })),
            ret_w,
            ret_s,
            None,
        ))
    }
}
