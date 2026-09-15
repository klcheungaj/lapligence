//! Dispatch.

use super::*;

impl<'a> Codegen<'a> {
    /// Render context for the IR built so far (the enclosing function, when
    /// any, resolves formal reads).
    pub(in super::super) fn render_ctx(&self) -> RCtx<'_> {
        RCtx {
            model: &self.model,
            func: self.cur_fn_ir.map(|i| &self.model.funcs[i]),
            sampled: false,
            activation_label: None,
        }
    }

    /// Render a lowered expression to its C code.
    pub(in super::super) fn render_ir_code(&self, ir: &IrExpr) -> Result<String, String> {
        let ctx = self.render_ctx();
        Ok(render_expr(&ctx, ir)?.code)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    pub(in super::super) fn lower_expr(
        &mut self,
        scope_path: &str,
        h: NodeId,
    ) -> Result<IrExpr, String> {
        if let Some(value) = self.lower_container_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_object_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.virtual_interface_member_expr(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.class_field_expr(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_enum_method(scope_path, h)? {
            return Ok(value);
        }
        if let NodeKind::MethodCall { name, .. } = self.kind(h) {
            if self.virtual_interface_method_info(h)?.is_some() {
                return self.lower_func_call_expr(scope_path, h, name, None);
            }
        }
        if matches!(self.kind(h), NodeKind::MethodCall { .. }) && self.is_class_method_call(h) {
            let (name, callee) = match self.kind(h) {
                NodeKind::MethodCall { name, callee, .. } => (name.clone(), *callee),
                _ => return Err("malformed class method call".to_owned()),
            };
            let receiver = self.class_method_receiver(h)?;
            let mut value = self.lower_func_call_expr(scope_path, h, &name, callee)?;
            if let IrExprKind::CallFn(call) = &mut value.kind {
                call.receiver = receiver;
                call.virtual_dispatch = self.class_method_virtual_dispatch(h);
            }
            return Ok(value);
        }
        match self.kind(h) {
            NodeKind::Expr(ExprKind::Constant { .. }) => {
                if let Some(comparison) = self.recover_folded_real_parameter_comparison(h) {
                    return Ok(comparison);
                }
                let c = self.const_of_node(h)?;
                Ok(IrExpr::new(
                    IrExprKind::Const(c.clone()),
                    c.width,
                    c.signed,
                    c.fill,
                ))
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(v)) => {
                    let c = val_to_const(v)?;
                    Ok(IrExpr::new(
                        IrExprKind::Const(c.clone()),
                        c.width,
                        c.signed,
                        None,
                    ))
                }
                Some(Val::Real(v)) => Ok(real_literal_expr(*v)),
                Some(Val::Str(_)) => Err("string enum constant in expression".to_string()),
                None => Err("enum constant without value in expression".to_string()),
            },
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                if let Some(info) = self.sampled_signal_of(*target) {
                    self.signal_read_expr(info)
                } else if let Some(info) = self.clocking_var_read_source_info(*target)? {
                    self.signal_read_expr(info)
                } else {
                    Err(format!(
                        "clocking block scope `{}` is not a value in `{scope_path}`",
                        self.node(*target).name
                    ))
                }
            }
            NodeKind::Expr(ExprKind::Ref { target }) => self.lower_ref_expr(scope_path, h, *target),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                self.ensure_clocking_readable(*base)?;
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{scope_path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.lower_expr(scope_path, *index)?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: vec![ie],
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, &[*index])?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i64::from(lsb);
                        return Ok(IrExpr::new(
                            IrExprKind::PartSel {
                                base: Box::new(self.signal_read_expr(&info)?),
                                left: right + i64::from(width) - 1,
                                right,
                            },
                            width,
                            false,
                            None,
                        ));
                    }
                }
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let ie = self.lower_member_select_index(scope_path, *base, *index)?;
                Ok(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(base_value),
                        idx: Box::new(ie),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                self.ensure_clocking_readable(*base)?;
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    return Ok(if member.two_state && !signal.real {
                        IrExpr::to_two_state(value)
                    } else {
                        value
                    });
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, indices)?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i64::from(lsb);
                    return Ok(IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: right + i64::from(width) - 1,
                            right,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{scope_path}` (base kind: {:?})",
                        self.node(*base).name,
                        self.kind(*base)
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if indices.len() == ndims + 1 {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (elem_sel, width) = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                            let r =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                            let (l, r, width) =
                                checked_select_bounds(l, r, "array-element part select")?;
                            (IrElemSel::Part(l, r), width)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect {
                            base_expr,
                            width_expr,
                            neg,
                            ..
                        }) => {
                            let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                            (
                                IrElemSel::Indexed {
                                    base: Box::new(
                                        self.lower_packed_index(scope_path, *base, *base_expr)?,
                                    ),
                                    width,
                                    negative: *neg ^ self.packed_range_ascending(*base),
                                },
                                width,
                            )
                        }
                        _ => {
                            let ie = self.lower_packed_index(scope_path, *base, last)?;
                            (IrElemSel::Bit(Box::new(ie)), 1)
                        }
                    };
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                Err(format!(
                    "array `{}` in `{scope_path}`: {}-level select on a \
                     {}-dimensional array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                self.ensure_clocking_readable(*base)?;
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                if let Some((info, member, lsb, width)) = self.packed_member_range_info(
                    *base,
                    self.eval_bound_i128(*left)?,
                    self.eval_bound_i128(*right)?,
                )? {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                let mut l = self.eval_bound_i128(*left)?;
                let mut r = self.eval_bound_i128(*right)?;
                if let Some((_, member)) = self.packed_member_info(*base) {
                    l = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        l,
                    )?);
                    r = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        r,
                    )?);
                } else {
                    l = self.packed_relative_bound(*base, l)?;
                    r = self.packed_relative_bound(*base, r)?;
                }
                let (l, r, width) = checked_select_bounds(l, r, "part select")?;
                Ok(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(base_value),
                        left: l,
                        right: r,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                self.ensure_clocking_readable(*base)?;
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let be = self.lower_packed_index(scope_path, *base, *base_expr)?;
                let we = self.lower_expr(scope_path, *width_expr)?;
                let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(base_value),
                        base_idx: Box::new(be),
                        width_expr: Box::new(we),
                        neg: *neg ^ self.packed_range_ascending(*base),
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) => {
                if streams.is_empty() {
                    return Err(format!("empty streaming concatenation in `{scope_path}`"));
                }
                let mut parts = Vec::with_capacity(streams.len());
                for stream in streams {
                    let value =
                        self.lower_stream_operand(scope_path, stream.value, stream.with_expr)?;
                    if value.is_real() {
                        return Err(format!(
                            "streaming concatenation of real value in `{scope_path}` is not supported"
                        ));
                    }
                    parts.push(value);
                }
                let runtime_sized = parts.iter().any(|part| part.width == LLG_MAX_WIDTH);
                let width = if runtime_sized {
                    Some(LLG_MAX_WIDTH)
                } else {
                    parts.iter().try_fold(0u32, |width, part| {
                        width
                            .checked_add(part.width)
                            .filter(|width| *width <= LLG_MAX_WIDTH)
                    })
                }
                .ok_or_else(|| {
                    format!("streaming concatenation in `{scope_path}` has an oversized operand")
                })?;
                let value = if let [value] = parts.as_slice() {
                    value.clone()
                } else {
                    IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
                };
                let slice = if *slice_size == 0 {
                    1
                } else {
                    (*slice_size).min(u64::from(width)) as u32
                };
                Ok(IrExpr::new(
                    IrExprKind::Stream {
                        value: Box::new(value),
                        slice,
                        direction: match direction {
                            DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                            DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                        },
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) if !*reordered
                && !*assignment
                && matches!(op, Operation::LogicalAnd | Operation::LogicalOr)
                && operands.len() == 2 =>
            {
                self.lower_logical_chain(scope_path, *op, operands)
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) => self.lower_operation(scope_path, *op, *reordered, *assignment, operands),
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
                propagated,
            }) => {
                if !cast_kind_known {
                    return Err(format!(
                        "cast kind cannot be determined without admitted source or semantic type metadata in `{scope_path}`"
                    ));
                }
                if matches!(ty.kind.as_str(), "real" | "shortreal") {
                    let v = self.lower_expr(scope_path, *operand)?;
                    return Ok(IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(v),
                            shortreal: ty.kind == "shortreal",
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    ));
                }
                let bitstream_source = self.lower_bitstream_source(scope_path, *operand)?;
                let source_value = match &bitstream_source {
                    Some(value) => value.clone(),
                    None => self.lower_expr(scope_path, *operand)?,
                };
                let target_width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or(ty.width);
                let (w, s) = match (target_width, ty.signed) {
                    (Some(w), s) => (w, if *size_cast { source_value.signed } else { s }),
                    (None, _) => {
                        return Err(format!(
                            "cast with unsized target type `{}` in `{scope_path}`",
                            ty.kind
                        ))
                    }
                };
                let v = source_value;
                if w > LLG_MAX_WIDTH {
                    return Err(format!(
                        "cast target in `{scope_path}` is {w} bits wide; the v1 \
                         runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                // Context propagation extends using the target signedness
                // (§11.8.2); assignment and explicit casts use the source (§11.8.3).
                let v = if *propagated {
                    let source_width = v.width;
                    IrExpr::resize_to(v, source_width, s)
                } else {
                    v
                };
                if let Some(source) = bitstream_source {
                    if source.width != w {
                        return Err(format!(
                            "bit-stream cast source is {} bits but target is {} bits in `{scope_path}`",
                            source.width, w
                        ));
                    }
                    return Ok(IrExpr::new(
                        IrExprKind::BitStreamCast {
                            a: Box::new(source),
                            source_width: w,
                            target_two_state: *two_state || is_two_state_kind(&ty.kind),
                        },
                        w,
                        s,
                        None,
                    ));
                }
                // Value-preserving conversion (LRM 1800-2009 §6.24.1: the
                // cast yields the value a variable of the cast type holds
                // after the assignment — extension follows the SOURCE's
                // signedness, so int'(8'hFF) is 255, not -1).
                ir_to_explicit_cast_storage(v, w, s, *two_state || is_two_state_kind(&ty.kind))
            }
            NodeKind::SysCall { name } => self.lower_sys_func_expr(scope_path, name, h),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                if *is_task {
                    return Err(format!(
                        "task call `{name}` used as an expression in `{scope_path}`"
                    ));
                }
                let mut value = self.lower_func_call_expr(scope_path, h, name, *callee)?;
                if let IrExprKind::CallFn(call) = &mut value.kind {
                    call.virtual_dispatch = self.class_method_virtual_dispatch(h);
                }
                Ok(value)
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "triggered" => {
                let target = self.event_target_of(*receiver).ok_or_else(|| {
                    format!(
                        "sequence `.triggered` status is not supported for an unresolved receiver in `{scope_path}`"
                    )
                })?;
                let event = self.event_ref_of(&target, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::EventTriggered(event),
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::MethodCall { name, .. } if name == "matched" => Err(format!(
                "sequence `.matched` status is not supported in `{scope_path}`"
            )),
            NodeKind::MethodCall { name, .. } if name == "triggered" => Err(format!(
                "sequence `.triggered` status is not supported in `{scope_path}`"
            )),
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let member_value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    return Ok(if member.two_state && !member_value.is_real() {
                        IrExpr::to_two_state(member_value)
                    } else {
                        member_value
                    });
                }
                if let Some((info, member)) = self.packed_member_info(h) {
                    let base = self.signal_read_expr(&info)?;
                    let member_value = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(base),
                            left: i64::from(member.lsb + member.width - 1),
                            right: i64::from(member.lsb),
                        },
                        member.width,
                        false,
                        None,
                    );
                    let selected = IrExpr::resize_to(member_value, member.width, member.signed);
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                // Interface and ordinary hierarchical members both resolve
                // to their concrete owned storage identity.
                if let Some(info) = self.hier_path_signal(h) {
                    return self.signal_read_expr(info);
                }
                Err(format!(
                    "hierarchical reference `{}` is not supported (in `{scope_path}`): {:?}",
                    self.node(h).name,
                    self.kind(h)
                ))
            }
            other => Err(format!(
                "unsupported expression in `{scope_path}` (node kind {other:?})"
            )),
        }
    }

    fn recover_folded_real_parameter_comparison(&self, node: NodeId) -> Option<IrExpr> {
        let NodeKind::Expr(ExprKind::Constant {
            source: ConstantSource::Exact(source),
            size: 1,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        let (left, op, right) = [
            ("!=", IrBinOp::Neq),
            ("==", IrBinOp::Eq),
            ("<=", IrBinOp::Le),
            (">=", IrBinOp::Ge),
            ("<", IrBinOp::Lt),
            (">", IrBinOp::Gt),
        ]
        .into_iter()
        .find_map(|(token, op)| {
            source
                .split_once(token)
                .map(|(left, right)| (left.trim(), op, right.trim()))
        })?;

        let mut scope = self.node(node).parent;
        let mut lexical_scopes = Vec::new();
        let scope = loop {
            let candidate = scope?;
            if matches!(
                self.kind(candidate),
                NodeKind::ModuleInst { .. } | NodeKind::GenScope
            ) {
                break candidate;
            }
            lexical_scopes.push(candidate);
            scope = self.node(candidate).parent;
        };
        let real_parameter = |name: &str| {
            let shadowed = lexical_scopes.iter().any(|scope| {
                self.node(*scope).children.iter().any(|declaration| {
                    self.node(*declaration).name == name
                        && matches!(
                            self.kind(*declaration),
                            NodeKind::Var { .. }
                                | NodeKind::Array { .. }
                                | NodeKind::Param { .. }
                                | NodeKind::FuncArg { .. }
                        )
                })
            });
            if shadowed {
                return None;
            }
            self.node(scope).children.iter().find_map(|parameter| {
                (self.node(*parameter).name == name)
                    .then(|| self.param_vals.get(parameter))
                    .flatten()
                    .and_then(|value| match value {
                        Val::Real(value) => Some(*value),
                        Val::Bits(_) | Val::Str(_) => None,
                    })
            })
        };
        let reverse = |op| match op {
            IrBinOp::Lt => IrBinOp::Gt,
            IrBinOp::Le => IrBinOp::Ge,
            IrBinOp::Gt => IrBinOp::Lt,
            IrBinOp::Ge => IrBinOp::Le,
            other => other,
        };
        if let (Some(parameter), Some(literal)) =
            (real_parameter(left), parse_decimal_real_literal(right))
        {
            return Some(cmp_expr_ir(
                op,
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        if let (Some(literal), Some(parameter)) =
            (parse_decimal_real_literal(left), real_parameter(right))
        {
            return Some(cmp_expr_ir(
                reverse(op),
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        None
    }

    fn lower_ref_expr(
        &mut self,
        scope_path: &str,
        r: NodeId,
        target: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(iterator) = self.container_iterator {
            if target == Some(iterator.node) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead("__llg_method_item".to_owned()),
                    iterator.item_width,
                    iterator.item_signed,
                    None,
                ));
            }
        }
        if let Some(target) = target {
            if let Some(binding) = self.assertion_local_binding(target)? {
                self.ensure_assertion_local_initializer(scope_path, target, binding)?;
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead(format!(
                        "llg_sequence_local_read(data, {}u)",
                        binding.slot
                    )),
                    binding.width,
                    binding.signed,
                    None,
                ));
            }
        }
        if let Some(captured) = self
            .capture_target(r)
            .or_else(|| target.filter(|target| self.capture_locals.contains_key(target)))
        {
            let binding = self
                .capture_binding(captured)
                .expect("capture target must have a binding");
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                binding.local.width,
                binding.local.signed,
                None,
            ));
        }
        if self.lexical_proc_string_local(r).is_some() {
            return Err(format!(
                "string procedural local `{}` cannot be used as a packed expression in `{scope_path}`",
                self.node(r).name
            ));
        }
        if let Some((_, info)) = self.lexical_proc_local(r) {
            if let Some(signal) = &info.static_signal {
                return self.signal_read_expr(signal);
            }
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(info.c_name.clone()),
                info.width,
                info.signed,
                None,
            ));
        }
        if let Some(t) = target {
            let t = self.canonical_func_target(t).unwrap_or(t);
            if let Some(binding) = self.capture_binding(t) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                    binding.local.width,
                    binding.local.signed,
                    None,
                ));
            }
            if self.unpacked_aggregates.contains_key(&t) {
                return Err(format!(
                    "whole unpacked aggregate `{}` is not supported in scalar expression `{scope_path}`",
                    self.node(t).name
                ));
            }
            if let Some(info) = self.sampled_signal_of(t) {
                return self.signal_read_expr(info);
            }
            if let Some(info) = self.clocking_var_read_source_info(t)? {
                return self.signal_read_expr(info);
            }
            if let Some(info) = self.signal_of(t) {
                return self.signal_read_expr(info);
            }
            if !self.proc_local_is_shadowed(r) {
                if let Some(info) = self.proc_local_info(t) {
                    if let Some(signal) = &info.static_signal {
                        return self.signal_read_expr(signal);
                    }
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(info.c_name.clone()),
                        info.width,
                        info.signed,
                        None,
                    ));
                }
            }
            // Function/task body reads: formals, locals and the return
            // variable (by arena node).
            if let Some(f) = &self.func {
                if let Some(ir) = f.arg_ir.get(&t) {
                    return Ok(ir.clone());
                }
                if let Some((cname, w, s, _, _shortreal)) = f.locals.get(&t) {
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        *w,
                        *s,
                        None,
                    ));
                }
                if f.ret_node == Some(t) {
                    if let Some(rctx) = &f.ret {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(v) = self.param_vals.get(&t) {
                return match v {
                    Val::Bits(b) => {
                        let c = val_to_const(b)?;
                        Ok(IrExpr::new(
                            IrExprKind::Const(c.clone()),
                            c.width,
                            c.signed,
                            None,
                        ))
                    }
                    Val::Real(value) => Ok(real_literal_expr(*value)),
                    Val::Str(value) => match self.kind(t) {
                        NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                            Some(width) => {
                                let c = string_to_const(value)?;
                                let expr = IrExpr::new(
                                    IrExprKind::Const(c.clone()),
                                    c.width,
                                    c.signed,
                                    None,
                                );
                                Ok(IrExpr::convert_to(expr, width, ty.signed))
                            }
                            None => Err(format!(
                                "string parameter `{}` used as a value is not supported",
                                self.node(t).name
                            )),
                        },
                        _ => Err(format!(
                            "string parameter `{}` used as a value is not supported",
                            self.node(t).name
                        )),
                    },
                };
            }
            if let NodeKind::EnumConst { value } = self.kind(t) {
                return enum_value_expr(value.as_ref(), &self.node(t).name);
            }
            return Err(format!(
                "cannot resolve bound expression reference `{}` in `{scope_path}`",
                self.node(r).name
            ));
        }
        // Unbound enum references can still arise in the flat definition
        // view. Any captured target identity must resolve above.
        let name = self.node(r).name.clone();
        if !name.is_empty() {
            // io_decls are not indexed, so formals resolve by name.
            if let Some(f) = &self.func {
                for (io, ir) in &f.arg_ir {
                    if self.node(*io).name == name {
                        return Ok(ir.clone());
                    }
                }
                for (node, (cname, w, s, _, _shortreal)) in &f.locals {
                    if self.node(*node).name == name {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(cname.clone()),
                            *w,
                            *s,
                            None,
                        ));
                    }
                }
                if let Some(rctx) = &f.ret {
                    if rctx
                        .node
                        .map(|n| self.node(n).name == name)
                        .unwrap_or(false)
                    {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return self.signal_read_expr(info);
            }
            // Some unqualified module-local enum uses can lack a resolved
            // target. Resolve those only against the current
            // instance's matching flat module definition and only when the
            // enumerator name is unique there.
            let def_name = match self.kind(self.inst) {
                NodeKind::ModuleInst { def_name, .. } => strip_lib(def_name),
                _ => String::new(),
            };
            let mut matches = self
                .db
                .flat_modules()
                .iter()
                .filter(|module| match self.kind(**module) {
                    NodeKind::ModuleInst {
                        def_name: candidate,
                        ..
                    } => strip_lib(candidate) == def_name,
                    _ => false,
                })
                .flat_map(|module| self.node(*module).children.iter())
                .filter_map(|candidate| match self.kind(*candidate) {
                    NodeKind::EnumConst { value } if self.node(*candidate).name == name => {
                        Some((value.as_ref(), self.node(*candidate).name.as_str()))
                    }
                    _ => None,
                });
            if let Some((value, enum_name)) = matches.next() {
                if matches.next().is_none() {
                    return enum_value_expr(value, enum_name);
                }
            }
        }
        Err(format!(
            "cannot resolve expression reference `{name}` in `{scope_path}`"
        ))
    }

    /// A plain constant node (`ExprKind::Constant`); used where the old code
    /// called `read_const` directly on a handle.
    pub(in super::super) fn const_of_node(&self, node: NodeId) -> Result<IrConst, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                time_scale,
                ..
            }) => {
                let mut c = if let Some(fill) = self.source_fill_literal(node) {
                    IrConst {
                        bits: vec![(fill == 1) as u64],
                        x: vec![(fill == 2) as u64],
                        z: vec![(fill == 3) as u64],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: Some(fill),
                    }
                } else {
                    read_const_from(value, *size)?
                };
                if *const_type == ConstantType::Time && self.round_time_literals {
                    let raw = c.real_value().ok_or_else(|| {
                        format!(
                            "time literal at {}:{}:{} has no real value",
                            self.node(node).file.as_deref().unwrap_or("<unknown>"),
                            self.node(node).line,
                            self.node(node).col
                        )
                    })?;
                    c = IrConst::real(self.rounded_time_literal(node, raw, source, *time_scale)?);
                }
                let (signed, literal_width) = self.signed_based_literal_info(node);
                if signed {
                    if let Some(width) = literal_width {
                        if width < c.width {
                            c = read_const_from(value, width as i32)?;
                        }
                    }
                    c.signed = true;
                }
                Ok(c)
            }
            _ => Err("unsupported constant value format".to_string()),
        }
    }

    pub(in super::super) fn indexed_part_select_width(
        &self,
        node: NodeId,
        scope_path: &str,
    ) -> Result<u32, String> {
        let value = self.eval_bound_i128(node).map_err(|_| {
            format!("indexed part-select width must be a constant in `{scope_path}`")
        })?;
        let width = u32::try_from(value)
            .map_err(|_| format!("indexed part-select width must be positive in `{scope_path}`"))?;
        if width == 0 {
            return Err(format!(
                "indexed part-select width must be positive in `{scope_path}`"
            ));
        }
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "indexed part-select width {width} exceeds maximum {LLG_MAX_WIDTH} in `{scope_path}`"
            ));
        }
        Ok(width)
    }
}
