//! Expression and assignment-target lowering into typed simulator IR.

use super::*;

impl<'a> Codegen<'a> {
    /// Render context for the IR built so far (the enclosing function, when
    /// any, resolves formal reads).
    fn render_ctx(&self) -> RCtx<'_> {
        RCtx {
            model: &self.model,
            func: self.cur_fn_ir.map(|i| &self.model.funcs[i]),
        }
    }

    /// Render a lowered expression to its C code.
    pub(super) fn render_ir_code(&self, ir: &IrExpr) -> Result<String, String> {
        let ctx = self.render_ctx();
        Ok(render_expr(&ctx, ir)?.code)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    pub(super) fn lower_expr(&mut self, scope_path: &str, h: NodeId) -> Result<IrExpr, String> {
        if let Some(value) = self.lower_container_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_object_query(scope_path, h)? {
            return Ok(value);
        }
        match self.kind(h) {
            NodeKind::Expr(ExprKind::Constant { .. }) => {
                if let Some(comparison) = self.recover_folded_real_parameter_comparison(h) {
                    return Ok(comparison);
                }
                if let Some(literal) = self.source_time_literal(h) {
                    let timescale = self.timescale_of_node(h);
                    let value = time_literal_to_real(&literal, timescale)?.ok_or_else(|| {
                        format!("invalid time literal `{literal}` in `{scope_path}`")
                    })?;
                    return Ok(real_literal_expr(value));
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
            NodeKind::Expr(ExprKind::Ref { target }) => self.lower_ref_expr(scope_path, h, *target),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(ai) = self.array_of(*base).cloned() {
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
                            arr: ai.ir,
                            indices: vec![ie],
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i64::from(lsb);
                        return Ok(IrExpr::new(
                            IrExprKind::PartSel {
                                base: Box::new(sig_read_expr_full(&info)),
                                left: right + i64::from(width) - 1,
                                right,
                            },
                            width,
                            false,
                            None,
                        ));
                    }
                }
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let ie = self.lower_expr(scope_path, *index)?;
                Ok(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(sig_read_expr_full(&info)),
                        idx: Box::new(ie),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i64::from(lsb);
                    return Ok(IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(sig_read_expr_full(&info)),
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
                        "cannot resolve array base of select `{}` in `{scope_path}`",
                        self.node(*base).name
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
                            arr: ai.ir,
                            indices: ies,
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if indices.len() == ndims + 1 {
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (elem_sel, width) = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l = self.eval_bound_i128(*left)?;
                            let r = self.eval_bound_i128(*right)?;
                            let (l, r, width) =
                                checked_select_bounds(l, r, "array-element part select")?;
                            (IrElemSel::Part(l, r), width)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect { .. }) => {
                            return Err(format!(
                                "indexed part-select on an array element is not \
                                 supported in `{scope_path}`"
                            ))
                        }
                        _ => {
                            let ie = self.lower_expr(scope_path, last)?;
                            (IrElemSel::Bit(Box::new(ie)), 1)
                        }
                    };
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: ai.ir,
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
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let l = self.eval_bound_i128(*left)?;
                let r = self.eval_bound_i128(*right)?;
                let (l, r, width) = checked_select_bounds(l, r, "part select")?;
                Ok(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(sig_read_expr_full(&info)),
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
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let be = self.lower_expr(scope_path, *base_expr)?;
                let we = self.lower_expr(scope_path, *width_expr)?;
                let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(sig_read_expr_full(&info)),
                        base_idx: Box::new(be),
                        width_expr: Box::new(we),
                        neg: *neg,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                operands,
            }) => self.lower_operation(scope_path, op.as_raw(), *reordered, operands),
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
            }) => {
                if !cast_kind_known {
                    return Err(format!(
                        "cast kind cannot be determined without admitted source or UHDM decompile in `{scope_path}`"
                    ));
                }
                let v = self.lower_expr(scope_path, *operand)?;
                if matches!(ty.kind.as_str(), "real" | "shortreal") {
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
                let target_width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or(ty.width);
                let (w, s) = match (target_width, ty.signed) {
                    (Some(w), s) => (w, if *size_cast { v.signed } else { s }),
                    (None, _) => {
                        return Err(format!(
                            "cast with unsized target type `{}` in `{scope_path}`",
                            ty.kind
                        ))
                    }
                };
                if w > LLG_MAX_WIDTH {
                    return Err(format!(
                        "cast target in `{scope_path}` is {w} bits wide; the v1 \
                         runtime maximum supported width is {LLG_MAX_WIDTH}"
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
            } => {
                if *is_task {
                    return Err(format!(
                        "task call `{name}` used as an expression in `{scope_path}`"
                    ));
                }
                self.lower_func_call_expr(scope_path, h, name, *callee)
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let member_value = IrExpr::resize_to(
                        sig_read_expr_full(&member_info.signal),
                        member.ty.width.ok_or_else(|| {
                            format!("unpacked member `{}` has unresolved width", member.name)
                        })?,
                        member.ty.signed,
                    );
                    if let Some(select) = self.packed_member_select(h)? {
                        let selected = match select {
                            PackedMemberSelect::Bit(index) => {
                                let index = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    index,
                                )?;
                                IrExpr::new(
                                    IrExprKind::BitSel {
                                        base: Box::new(member_value),
                                        idx: Box::new(lhs_integer_expr(i128::from(index))),
                                    },
                                    1,
                                    false,
                                    None,
                                )
                            }
                            PackedMemberSelect::Part(left, right) => {
                                let left = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    left,
                                )?;
                                let right = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    right,
                                )?;
                                let (left, right, width) = checked_select_bounds(
                                    i128::from(left),
                                    i128::from(right),
                                    "unpacked aggregate member part select",
                                )?;
                                IrExpr::new(
                                    IrExprKind::PartSel {
                                        base: Box::new(member_value),
                                        left,
                                        right,
                                    },
                                    width,
                                    false,
                                    None,
                                )
                            }
                        };
                        return Ok(if member.two_state {
                            IrExpr::to_two_state(selected)
                        } else {
                            selected
                        });
                    }
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(member_value)
                    } else {
                        member_value
                    });
                }
                if let Some((info, member)) = self.packed_member_info(h) {
                    let base = sig_read_expr_full(&info);
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
                    if let Some(select) = self.packed_member_select(h)? {
                        let selected = match select {
                            PackedMemberSelect::Bit(index) => {
                                let index = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    index,
                                )?;
                                IrExpr::new(
                                    IrExprKind::BitSel {
                                        base: Box::new(member_value),
                                        idx: Box::new(lhs_integer_expr(i128::from(index))),
                                    },
                                    1,
                                    false,
                                    None,
                                )
                            }
                            PackedMemberSelect::Part(left, right) => {
                                let left = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    left,
                                )?;
                                let right = self.aggregate_member_relative_bound(
                                    &member.name,
                                    &member.packed_ranges,
                                    right,
                                )?;
                                let (left, right, width) = checked_select_bounds(
                                    i128::from(left),
                                    i128::from(right),
                                    "packed-member part select",
                                )?;
                                IrExpr::new(
                                    IrExprKind::PartSel {
                                        base: Box::new(member_value),
                                        left,
                                        right,
                                    },
                                    width,
                                    false,
                                    None,
                                )
                            }
                        };
                        return Ok(if member.two_state {
                            IrExpr::to_two_state(selected)
                        } else {
                            selected
                        });
                    }
                    let selected = IrExpr::resize_to(member_value, member.width, member.signed);
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                // 2-part interface member access (`m.data`): a read of the
                // resolved per-port copy var.
                if let Some(info) = self.hier_path_signal(h) {
                    return Ok(sig_read_expr_full(info));
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
        if let Some((_, info)) = self.lexical_proc_local(r) {
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(info.c_name.clone()),
                info.width,
                info.signed,
                None,
            ));
        }
        if let Some(t) = target {
            if self.unpacked_aggregates.contains_key(&t) {
                return Err(format!(
                    "whole unpacked aggregate `{}` is not supported in scalar expression `{scope_path}`",
                    self.node(t).name
                ));
            }
            if let Some(info) = self.signal_of(t) {
                return Ok(sig_read_expr_full(info));
            }
            if !self.proc_local_is_shadowed(r) {
                if let Some(info) = self.proc_locals.get(&t) {
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
                if let Some((cname, w, s, _)) = f.locals.get(&t) {
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
        }
        // Name fallback within the current scope.
        let name = self.node(r).name.clone();
        if !name.is_empty() {
            // io_decls are not indexed, so formals resolve by name.
            if let Some(f) = &self.func {
                for (io, ir) in &f.arg_ir {
                    if self.node(*io).name == name {
                        return Ok(ir.clone());
                    }
                }
                for (node, (cname, w, s, _)) in &f.locals {
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
                return Ok(sig_read_expr_full(info));
            }
            // Surelog v1.87 leaves some unqualified module-local enum uses
            // without `vpiActual`.  Resolve those only against the current
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
    pub(super) fn const_of_node(&self, node: NodeId) -> Result<IrConst, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                if let Some(literal) = self.source_time_literal(node) {
                    return Err(format!(
                        "time literal `{literal}` requires a runtime expression context"
                    ));
                }
                if let Some(literal) = self.embedded_source_time_literal(node) {
                    return Err(format!(
                        "folded expression containing time literal `{literal}` is not supported"
                    ));
                }
                if self.unverified_time_literal_candidate(node) {
                    return Err(
                        "cannot verify unsigned constant source after possible time-literal rewriting"
                            .to_owned(),
                    );
                }
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

    pub(super) fn indexed_part_select_width(
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

    /// Recover a time literal only when the UHDM constant's exact source span
    /// is one complete token. Surelog v1.87 rewrites these constants to an
    /// unsigned femtosecond payload and loses `vpiTimeConst`; accepting a
    /// prefix or a folded compound span would silently invent the wrong unit.
    pub(super) fn source_time_literal(&self, node: NodeId) -> Option<String> {
        let NodeKind::Expr(ExprKind::Constant {
            source: ConstantSource::Exact(token),
            const_type: ConstantType::UnsignedInteger,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        if time_literal_token(token).is_some_and(|literal| literal == token) {
            Some(token.clone())
        } else {
            None
        }
    }

    pub(super) fn embedded_source_time_literal(&self, node: NodeId) -> Option<String> {
        let NodeKind::Expr(ExprKind::Constant {
            source: ConstantSource::Exact(source),
            const_type: ConstantType::UnsignedInteger,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        time_literal_token(source).map(str::to_owned)
    }

    pub(super) fn unverified_time_literal_candidate(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                source: ConstantSource::Unavailable | ConstantSource::NotCaptured,
                time_literal_candidate: true,
                ..
            })
                | NodeKind::Expr(ExprKind::Constant {
                    source: ConstantSource::Exact(_),
                    time_literal_candidate: true,
                    ..
                }) if self.source_time_literal(node).is_none()
                    && self.embedded_source_time_literal(node).is_none()
        )
    }

    pub(super) fn time_literal_in_subtree(&self, node: NodeId) -> Option<String> {
        if let Some(literal) = self.source_time_literal(node) {
            return Some(literal);
        }
        if let Some(literal) = self.embedded_source_time_literal(node) {
            return Some(literal);
        }
        for child in &self.node(node).children {
            if let Some(literal) = self.time_literal_in_subtree(*child) {
                return Some(literal);
            }
        }
        None
    }

    pub(super) fn unverified_time_literal_in_subtree(&self, node: NodeId) -> bool {
        self.unverified_time_literal_candidate(node)
            || self
                .node(node)
                .children
                .iter()
                .any(|child| self.unverified_time_literal_in_subtree(*child))
    }

    /// Lower one operation, mirroring the pre-IR emitter's operand shapes,
    /// result widths/signedness and error strings arm-for-arm.
    fn lower_operation(
        &mut self,
        scope_path: &str,
        otype: i32,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        use vpi::*;
        macro_rules! op {
            ($i:expr) => {
                self.lower_expr(scope_path, operands[$i])?
            };
        }
        let maxw = |a: &IrExpr, b: &IrExpr| a.width.max(b.width);

        match otype {
            vpiAddOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Add, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Add, a, b, scope_path)
            }
            vpiSubOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Sub, a, b, scope_path)
            }
            vpiMultOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Mul, a, b, scope_path)
            }
            vpiDivOp | vpiModOp | vpiPowerOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    let rop = match otype {
                        vpiDivOp => IrRealBinOp::Div,
                        vpiModOp => IrRealBinOp::Mod,
                        _ => IrRealBinOp::Pow,
                    };
                    return Ok(real_bin_expr(rop, a, b));
                }
                let f = match otype {
                    vpiDivOp => IrBinOp::Div,
                    vpiModOp => IrBinOp::Mod,
                    _ => IrBinOp::Pow,
                };
                if matches!(f, IrBinOp::Div | IrBinOp::Mod) {
                    common_bin_expr_with_context(f, a, b, scope_path)
                } else {
                    let (width, signed) = (a.width, a.signed);
                    Ok(IrExpr::new(
                        IrExprKind::Bin {
                            op: f,
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        width,
                        signed,
                        None,
                    ))
                }
            }
            vpiBitAndOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitAnd, a, b, scope_path)
            }
            vpiBitOrOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitOr, a, b, scope_path)
            }
            vpiBitXorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXor, a, b, scope_path)
            }
            vpiBitXNorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXNor, a, b, scope_path)
            }
            vpiLogAndOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogAnd, a, b))
            }
            vpiLogOrOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogOr, a, b))
            }
            vpiEqOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Eq, a, b, scope_path)
            }
            vpiNeqOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Neq, a, b, scope_path)
            }
            vpiCaseEqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseEq, a, b, scope_path)
            }
            vpiCaseNeqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseNeq, a, b, scope_path)
            }
            vpiWildEqOp | vpiWildNeqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "wildcard equality on real value in `{scope_path}` is not supported"
                    ));
                }
                let width = maxw(&a, &b);
                let signed = a.signed && b.signed;
                let a = wildcard_operand_with_context(a, width, signed, scope_path)?;
                let b = wildcard_operand_with_context(b, width, signed, scope_path)?;
                let op = if otype == vpiWildEqOp {
                    IrBinOp::WildEq
                } else {
                    IrBinOp::WildNeq
                };
                Ok(cmp_expr_ir(op, a, b))
            }
            vpiLtOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Lt, a, b, scope_path)
            }
            vpiLeOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Le, a, b, scope_path)
            }
            vpiGtOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Gt, a, b, scope_path)
            }
            vpiGeOp => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Ge, a, b, scope_path)
            }
            vpiLShiftOp | vpiRShiftOp | vpiArithLShiftOp | vpiArithRShiftOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "shift on real value in `{scope_path}` is not supported"
                    ));
                }
                let f = match otype {
                    vpiLShiftOp => IrBinOp::Shl,
                    vpiRShiftOp => IrBinOp::Shr,
                    vpiArithLShiftOp => IrBinOp::Ashl,
                    _ => IrBinOp::Ashr,
                };
                let (w, s) = (a.width, a.signed);
                Ok(IrExpr::new(
                    IrExprKind::Bin {
                        op: f,
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiConditionOp => {
                let sel = op!(0);
                let a = op!(1);
                let b = op!(2);
                let (w, s) = if a.is_real() || b.is_real() {
                    (REAL_EXPR_WIDTH, true)
                } else {
                    (maxw(&a, &b), a.signed && b.signed)
                };
                let (a, b) = if w == REAL_EXPR_WIDTH {
                    (a, b)
                } else {
                    (
                        checked_operand_with_context(a, w, s, scope_path, "conditional context")?,
                        checked_operand_with_context(b, w, s, scope_path, "conditional context")?,
                    )
                };
                Ok(IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(sel),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiMinusOp => {
                let a = op!(0);
                let w = a.width;
                if a.is_real() {
                    return Ok(real_un_expr(a));
                }
                // Unary minus of an unsized decimal literal (`-3`): Surelog
                // represents the literal as an unsigned 64-bit UInt constant,
                // dropping the LRM signedness (unsized decimal literals are
                // signed, LRM 5.7.1).  Restore it so `$display("%d", -3)`
                // prints "-3" instead of the unsigned wrap.  Sized radix
                // literals (`-4'h3`) stay unsigned per the LRM.
                let signed_lit = matches!(
                    self.kind(operands[0]),
                    NodeKind::Expr(ExprKind::Constant {
                        value: ValueData::UInt(_),
                        ..
                    })
                ) && !a.signed;
                let s = a.signed;
                let neg = IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::Neg,
                        a: Box::new(a),
                    },
                    w,
                    s && !signed_lit,
                    None,
                );
                if signed_lit {
                    Ok(IrExpr::resize_to(neg, w, true))
                } else {
                    Ok(neg)
                }
            }
            vpiPlusOp => {
                let a = op!(0);
                Ok(a)
            }
            vpiNotOp => {
                let a = op!(0);
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(a),
                    },
                    1,
                    false,
                    None,
                ))
            }
            vpiBitNegOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "bitwise negation of real value in `{scope_path}` is not supported"
                    ));
                }
                let w = a.width;
                let s = a.signed;
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::BitNeg,
                        a: Box::new(a),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiUnaryAndOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedAnd, a))
            }
            vpiUnaryNandOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNand, a))
            }
            vpiUnaryOrOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedOr, a))
            }
            vpiUnaryNorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNor, a))
            }
            vpiUnaryXorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXor, a))
            }
            vpiUnaryXNorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXNor, a))
            }
            vpiStreamLROp | vpiStreamRLOp => {
                let (slice, value_node) = match operands {
                    [value] => (None, *value),
                    [slice, value] => {
                        let slice_value = self.eval_bits(*slice).map_err(|error| {
                            format!(
                                "streaming slice size must be a positive constant in \
                                 `{scope_path}`: {error}"
                            )
                        })?;
                        if slice_value.is_unknown() {
                            return Err(format!(
                                "streaming slice size must be known in `{scope_path}`"
                            ));
                        }
                        if slice_value.signed
                            && slice_value.width() != 0
                            && slice_value.bit_lsb(slice_value.width() - 1) == Bit::One
                        {
                            return Err(format!(
                                "streaming slice size must be positive in `{scope_path}`"
                            ));
                        }
                        let slice = slice_value.to_u128().unwrap_or(u128::MAX);
                        if slice == 0 {
                            return Err(format!(
                                "streaming slice size must be positive in `{scope_path}`"
                            ));
                        }
                        (Some(slice), *value)
                    }
                    _ => {
                        return Err(format!(
                            "malformed streaming concatenation in `{scope_path}`"
                        ));
                    }
                };
                let value = self.lower_expr(scope_path, value_node)?;
                if value.is_real() {
                    return Err(format!(
                        "streaming concatenation of real value in `{scope_path}` is not supported"
                    ));
                }
                let width = value.width;
                let slice = slice
                    .unwrap_or(1)
                    .min(u128::from(width))
                    .try_into()
                    .expect("packed stream width fits u32");
                Ok(IrExpr::new(
                    IrExprKind::Stream {
                        value: Box::new(value),
                        slice,
                        direction: if otype == vpiStreamLROp {
                            IrStreamDirection::LeftToRight
                        } else {
                            IrStreamDirection::RightToLeft
                        },
                    },
                    width,
                    false,
                    None,
                ))
            }
            vpiInsideOp => {
                let Some((value_node, item_nodes)) = operands.split_first() else {
                    return Err(format!("empty inside expression in `{scope_path}`"));
                };
                if item_nodes.is_empty() {
                    return Err(format!("inside set is empty in `{scope_path}`"));
                }
                let value = self.lower_expr(scope_path, *value_node)?;
                if value.is_real() {
                    return Err(format!(
                        "real-valued inside selector is not supported in `{scope_path}`"
                    ));
                }
                let shapes = item_nodes
                    .iter()
                    .map(|node| match self.kind(*node) {
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::List,
                            operands,
                            ..
                        }) if matches!(operands.len(), 1 | 2) => Ok(operands.clone()),
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::List,
                            ..
                        }) => Err(format!("malformed inside set item in `{scope_path}`")),
                        _ => Ok(vec![*node]),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut items = Vec::with_capacity(shapes.len());
                for shape in shapes {
                    match shape.as_slice() {
                        [item] => {
                            let item = self.lower_expr(scope_path, *item)?;
                            if item.is_real() {
                                return Err(format!(
                                    "real-valued inside item is not supported in `{scope_path}`"
                                ));
                            }
                            items.push(IrInsideItem::Value(item));
                        }
                        [low, high] => {
                            let low = self.lower_expr(scope_path, *low)?;
                            let high = self.lower_expr(scope_path, *high)?;
                            if low.is_real() || high.is_real() {
                                return Err(format!(
                                    "real-valued inside range is not supported in `{scope_path}`"
                                ));
                            }
                            items.push(IrInsideItem::Range { low, high });
                        }
                        _ => unreachable!("inside item shape validated above"),
                    }
                }
                Ok(IrExpr::new(
                    IrExprKind::Inside {
                        value: Box::new(value),
                        items,
                    },
                    1,
                    false,
                    None,
                ))
            }
            vpiConcatOp => {
                let mut parts = Vec::new();
                for operand in operands {
                    parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if reordered {
                    parts.reverse();
                }
                if parts.is_empty() {
                    return Err(format!("empty concatenation in `{scope_path}`"));
                }
                if parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "concatenation of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut width = 0u32;
                for p in &parts {
                    width += p.width;
                }
                if width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "concatenation in `{scope_path}` is {width} bits wide; \
                         the v1 runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Concat { parts },
                    width,
                    false,
                    None,
                ))
            }
            vpiMultiConcatOp => {
                let count = {
                    let value = self.eval_bits(operands[0])?;
                    if value.is_unknown() {
                        return Err(format!("unknown replication count in `{scope_path}`"));
                    }
                    value
                        .to_u128()
                        .and_then(|count| u64::try_from(count).ok())
                        .ok_or_else(|| {
                            format!("replication count does not fit in u64 in `{scope_path}`")
                        })?
                };
                let mut pat_parts = Vec::new();
                for operand in operands.iter().skip(1) {
                    pat_parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if pat_parts.is_empty() {
                    return Err(format!("empty replication in `{scope_path}`"));
                }
                if pat_parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "replication of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut pwidth = pat_parts[0].width as u128;
                for p in &pat_parts[1..] {
                    pwidth += p.width as u128;
                }
                let total = pwidth * count as u128;
                if total > LLG_MAX_WIDTH as u128 {
                    return Err(format!(
                        "replication in `{scope_path}` is {total} bits wide; \
                         the v1 runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Replicate {
                        count,
                        parts: pat_parts,
                    },
                    total as u32,
                    false,
                    None,
                ))
            }
            vpiCastOp => Err(format!(
                "cast expressions are not supported in `{scope_path}` \
                 (the database does not capture the cast typespec)"
            )),
            vpiMinTypMaxOp => {
                let a = op!(0);
                Ok(a)
            }
            other => Err(format!(
                "unsupported operation op type {other} in `{scope_path}`"
            )),
        }
    }

    /// Lower system-function expressions ($clog2/$time/$stime/$bits/$signed/
    /// $unsigned); timescale scaling happens here.
    fn lower_sys_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args: Vec<NodeId> = self.node(call).children.clone();
        match name {
            "$rtoi" | "$itor" | "$realtobits" | "$bitstoreal" | "$shortrealtobits"
            | "$bitstoshortreal" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                match name {
                    "$rtoi" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::Rtoi(Box::new(arg))),
                        32,
                        true,
                        None,
                    )),
                    "$itor" => {
                        let arg = if arg.is_real() {
                            IrExpr::new(
                                IrExprKind::CastToPacked { a: Box::new(arg) },
                                32,
                                true,
                                None,
                            )
                        } else {
                            arg
                        };
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::Itor(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$realtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::RealToBits(Box::new(arg))),
                        64,
                        false,
                        None,
                    )),
                    "$bitstoreal" => {
                        if arg.is_real() || arg.width != 64 {
                            return Err(format!(
                                "$bitstoreal requires an exactly 64-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                    "$shortrealtobits" => Ok(IrExpr::new(
                        IrExprKind::SysFunc(IrSysFunc::ShortRealToBits(Box::new(arg))),
                        32,
                        false,
                        None,
                    )),
                    _ => {
                        if arg.is_real() || arg.width != 32 {
                            return Err(format!(
                                "$bitstoshortreal requires an exactly 32-bit packed argument in `{scope_path}`"
                            ));
                        }
                        Ok(IrExpr::new(
                            IrExprKind::SysFunc(IrSysFunc::BitsToShortReal(Box::new(arg))),
                            0,
                            true,
                            None,
                        ))
                    }
                }
            }
            "$countones" | "$onehot" | "$onehot0" | "$isunknown" => {
                let [arg] = args.as_slice() else {
                    return Err(format!(
                        "{name} requires exactly one argument in `{scope_path}`"
                    ));
                };
                let arg = self.lower_expr(scope_path, *arg)?;
                if arg.is_real() {
                    return Err(format!(
                        "{name} requires a packed integral argument in `{scope_path}`"
                    ));
                }
                let kind = match name {
                    "$countones" => IrBitQuery::CountOnes,
                    "$onehot" => IrBitQuery::OneHot,
                    "$onehot0" => IrBitQuery::OneHot0,
                    _ => IrBitQuery::IsUnknown,
                };
                let (width, signed) = kind.result_type();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::BitQuery {
                        kind,
                        arg: Box::new(arg),
                    }),
                    width,
                    signed,
                    None,
                ))
            }
            "$clog2" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$clog2 without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "clog2 on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Clog2(Box::new(a))),
                    32,
                    false,
                    None,
                ))
            }
            "$time" | "$stime" => {
                // Both functions return the current time in the CALLING
                // module's unit; `$stime` is the 32-bit form. `llg_time()` is
                // in design-precision ticks (1 tick = design_precision_ps
                // ps), so now_ps = now * P.
                let unit_ps = self.timescale_of_node(call).unit_ps;
                let kind = if name == "$stime" {
                    IrTimeKind::STime
                } else {
                    IrTimeKind::Time
                };
                let width = kind.width();
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Time {
                        precision_ps: self.design_precision_ps,
                        unit_ps,
                        kind,
                    }),
                    width,
                    false,
                    None,
                ))
            }
            "$bits" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$bits without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "bits on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Bits(Box::new(a))),
                    32,
                    true,
                    None,
                ))
            }
            "$signed" | "$unsigned" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{name} without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "{name} on real value in `{scope_path}` is not supported"
                    ));
                }
                let s = name == "$signed";
                let w = a.width;
                Ok(IrExpr::resize_to(a, w, s))
            }
            _ => Err(format!(
                "unsupported system function {name} in `{scope_path}`"
            )),
        }
    }

    /// Lower an assignment LHS: the pre-IR [`Self::analyze_lhs`] decisions
    /// converted to [`IrLhs`] (identical by construction during the seam
    /// transition; sub-expression codes ride along verbatim).
    pub(super) fn lower_lhs(&mut self, path: &str, lhs: NodeId) -> Result<IrLhs, String> {
        let lh = self.analyze_lhs(path, lhs)?;
        self.lhs_to_ir(lh)
    }

    pub(super) fn lower_packed_aggregate_pattern(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        op: i32,
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == vpi::vpiAssignmentPatternOp
        ) {
            return Ok(None);
        }
        let target = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::Var { .. } => lhs,
            _ => return Ok(None),
        };
        let Some(layout) = self.db.aggregate_layout(target) else {
            return Ok(None);
        };
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return Ok(None);
        }
        if op != 0 && op != vpi::vpiAssignmentOp {
            return Err(format!(
                "compound assignment of packed aggregate pattern in `{path}` is not supported"
            ));
        }
        Ok(Some(
            self.lower_packed_aggregate_pattern_value(path, rhs, layout)?,
        ))
    }

    fn lower_packed_aggregate_pattern_value(
        &mut self,
        path: &str,
        rhs: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<IrExpr, String> {
        let values = self.aggregate_pattern_values(path, rhs, layout)?;
        let mut members = Vec::with_capacity(values.len());
        for (member_index, value_node) in values {
            let member = layout.members.get(member_index).ok_or_else(|| {
                format!("aggregate pattern member index {member_index} is out of bounds")
            })?;
            let width = member.ty.width.ok_or_else(|| {
                format!(
                    "packed member `{}` has unresolved width in `{path}`",
                    member.name
                )
            })?;
            let value = if let Some(nested) = member.aggregate.as_deref() {
                if matches!(
                    self.kind(value_node),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == vpi::vpiAssignmentPatternOp
                ) {
                    if !matches!(
                        nested.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) {
                        return Err(format!(
                            "nested unpacked aggregate member `{}` in `{path}` is not supported",
                            member.name
                        ));
                    }
                    self.lower_packed_aggregate_pattern_value(path, value_node, nested)?
                } else {
                    self.lower_expr(path, value_node)?
                }
            } else {
                self.lower_expr(path, value_node)?
            };
            members.push(ir_to_storage(
                value,
                width,
                member.ty.signed,
                member.two_state,
            )?);
        }
        let value = if layout.kind == AggregateKind::PackedUnion {
            members
                .into_iter()
                .next()
                .ok_or_else(|| format!("packed union assignment pattern is empty in `{path}`"))?
        } else {
            let width = members
                .iter()
                .try_fold(0u32, |total, member| total.checked_add(member.width()));
            let width = width.ok_or_else(|| {
                format!("packed aggregate assignment pattern width overflows in `{path}`")
            })?;
            IrExpr::new(IrExprKind::Concat { parts: members }, width, false, None)
        };
        Ok(value)
    }

    pub(super) fn lower_unpacked_aggregate_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: i32,
    ) -> Result<Option<IrStmt>, String> {
        let lhs_aggregate = self.unpacked_aggregate_info(lhs);
        let rhs_aggregate = self.unpacked_aggregate_info(rhs);
        let rhs_is_pattern = matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == vpi::vpiAssignmentPatternOp
        );
        if lhs_aggregate.is_none() && rhs_aggregate.is_none() {
            return Ok(None);
        }
        let (lhs_target, lhs_aggregate) = lhs_aggregate.ok_or_else(|| {
            format!("unpacked aggregate used as a scalar assignment RHS in `{path}`")
        })?;
        if rhs_is_pattern {
            if op != 0 && op != vpi::vpiAssignmentOp {
                return Err(format!(
                    "compound assignment of unpacked aggregate pattern in `{path}` is not supported"
                ));
            }
            let layout = self.db.aggregate_layout(lhs_target).ok_or_else(|| {
                format!(
                    "unpacked aggregate `{}` in `{path}` has no captured layout",
                    self.node(lhs_target).name
                )
            })?;
            let values = self.aggregate_pattern_values(path, rhs, layout)?;
            let mut assignments = Vec::with_capacity(values.len());
            for (member_index, value) in values {
                let left = lhs_aggregate.members.get(member_index).ok_or_else(|| {
                    format!("aggregate pattern member index {member_index} is out of bounds")
                })?;
                let width = left.member.ty.width.ok_or_else(|| {
                    format!(
                        "unpacked member `{}` has unresolved width",
                        left.member.name
                    )
                })?;
                let value = self.lower_expr(path, value)?;
                let lhs = IrLhs::Part(
                    left.signal.ir,
                    i64::from(width - 1),
                    0,
                    left.member.two_state,
                );
                let value = apply_lhs_assignment_context(&self.model, &lhs, value);
                assignments.push(IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba,
                });
            }
            return Ok(Some(IrStmt::Block(assignments)));
        }
        if lhs_aggregate.kind == AggregateKind::UnpackedStruct
            && matches!(self.kind(rhs), NodeKind::Expr(ExprKind::Constant { .. }))
        {
            // Surelog folds constant unpacked-structure assignment patterns
            // to one integral payload. Recover the positional member values
            // from the standard first-member-most-significant layout.
            let total_width = lhs_aggregate
                .members
                .iter()
                .try_fold(0u32, |width, member| {
                    width.checked_add(member.member.ty.width?)
                })
                .ok_or_else(|| format!("unpacked struct pattern width overflow in `{path}`"))?;
            let packed = IrExpr::convert_to(self.lower_expr(path, rhs)?, total_width, false);
            let mut right = 0u32;
            let mut values = Vec::with_capacity(lhs_aggregate.members.len());
            for member in lhs_aggregate.members.iter().rev() {
                let width = member.member.ty.width.ok_or_else(|| {
                    format!(
                        "unpacked member `{}` has unresolved width",
                        member.member.name
                    )
                })?;
                values.push((member, right, width));
                right = right.checked_add(width).ok_or_else(|| {
                    format!("unpacked struct pattern offset overflow in `{path}`")
                })?;
            }
            values.reverse();
            let mut assignments = Vec::with_capacity(values.len());
            for (left, right, width) in values {
                let value = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(packed.clone()),
                        left: i64::from(right + width - 1),
                        right: i64::from(right),
                    },
                    width,
                    false,
                    None,
                );
                let lhs = IrLhs::Part(
                    left.signal.ir,
                    i64::from(width - 1),
                    0,
                    left.member.two_state,
                );
                let value = apply_lhs_assignment_context(&self.model, &lhs, value);
                assignments.push(IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba,
                });
            }
            return Ok(Some(IrStmt::Block(assignments)));
        }
        let (rhs_target, rhs_aggregate) = rhs_aggregate.ok_or_else(|| {
            format!("unpacked aggregate used as a scalar assignment LHS in `{path}`")
        })?;
        if op != 0 && op != vpi::vpiAssignmentOp {
            return Err(format!(
                "compound assignment of unpacked aggregates in `{path}` is not supported"
            ));
        }
        let same_type_identity = match (
            lhs_aggregate.type_identity.as_deref(),
            rhs_aggregate.type_identity.as_deref(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => lhs_target == rhs_target,
            _ => false,
        };
        let compatible = same_type_identity
            && lhs_aggregate.kind == rhs_aggregate.kind
            && lhs_aggregate.members.len() == rhs_aggregate.members.len()
            && lhs_aggregate
                .members
                .iter()
                .zip(&rhs_aggregate.members)
                .all(|(left, right)| {
                    left.member.name == right.member.name
                        && left.member.ty == right.member.ty
                        && left.member.two_state == right.member.two_state
                        && left.member.packed_ranges == right.member.packed_ranges
                });
        if !compatible {
            return Err(format!(
                "assignment between incompatible unpacked aggregate types `{}` and `{}` in `{path}`",
                self.node(lhs_target).name,
                self.node(rhs_target).name
            ));
        }
        if lhs_aggregate.kind == AggregateKind::UnpackedUnion {
            let lhs = lhs_aggregate.members.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(lhs_target).name
                )
            })?;
            let rhs = rhs_aggregate.members.first().ok_or_else(|| {
                format!(
                    "unpacked union `{}` has no members",
                    self.node(rhs_target).name
                )
            })?;
            return Ok(Some(IrStmt::Assign {
                lhs: IrLhs::Whole(lhs.signal.ir),
                rhs: sig_read_expr_full(&rhs.signal),
                nba,
            }));
        }
        let mut assignments = Vec::with_capacity(lhs_aggregate.members.len());
        for (left, right) in lhs_aggregate.members.iter().zip(&rhs_aggregate.members) {
            let width = left.member.ty.width.ok_or_else(|| {
                format!(
                    "unpacked member `{}` has unresolved width",
                    left.member.name
                )
            })?;
            let mut value = IrExpr::resize_to(
                sig_read_expr_full(&right.signal),
                width,
                right.member.ty.signed,
            );
            if right.member.two_state {
                value = IrExpr::to_two_state(value);
            }
            let lhs = IrLhs::Part(
                left.signal.ir,
                i64::from(width - 1),
                0,
                left.member.two_state,
            );
            let value = apply_lhs_assignment_context(&self.model, &lhs, value);
            assignments.push(IrStmt::Assign {
                lhs,
                rhs: value,
                nba,
            });
        }
        Ok(Some(IrStmt::Block(assignments)))
    }

    /// Convert a pre-IR [`Lhs`] to its [`IrLhs`] form using the registered
    /// model indices.
    fn lhs_to_ir(&self, lh: Lhs) -> Result<IrLhs, String> {
        Ok(match lh {
            Lhs::Whole(info) => IrLhs::Whole(info.ir),
            Lhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
            } => IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
            },
            Lhs::Bit(info, index, two_state) => IrLhs::Bit(info.ir, index, two_state),
            Lhs::Part(info, left, right, two_state) => {
                let (left, right, _) =
                    checked_select_bounds(left, right, "assignment part select")?;
                IrLhs::Part(info.ir, left, right, two_state)
            }
            Lhs::IdxPart(info, base, width_expr, width, neg, two_state) => {
                IrLhs::IdxPart(info.ir, base, width_expr, width, neg, two_state)
            }
            Lhs::ArrayElem(ae) => IrLhs::ArrayElem {
                arr: ae.arr.ir,
                indices: ae.indices,
                elem_sel: match ae.elem_sel {
                    ElemSel::Whole => IrElemSel::Whole,
                    ElemSel::Part(l, r) => {
                        let (left, right, _) =
                            checked_select_bounds(l, r, "array-element assignment part select")?;
                        IrElemSel::Part(left, right)
                    }
                    ElemSel::Bit(index) => IrElemSel::Bit(Box::new(index)),
                },
            },
            Lhs::Stream {
                parts,
                slice,
                direction,
            } => {
                let mut ir_parts = Vec::with_capacity(parts.len());
                let mut width = 0u32;
                for part in parts {
                    let part = self.lhs_to_ir(part)?;
                    let part_width = packed_lhs_width(&self.model, &part).ok_or_else(|| {
                        "streaming assignment target has no packed width".to_string()
                    })?;
                    width = width.checked_add(part_width).ok_or_else(|| {
                        "streaming assignment target width exceeds the supported range".to_string()
                    })?;
                    if width > LLG_MAX_WIDTH {
                        return Err(format!(
                            "streaming assignment target is {width} bits wide; the v1 runtime \
                             maximum supported width is {LLG_MAX_WIDTH}"
                        ));
                    }
                    ir_parts.push((part, part_width));
                }
                if width == 0 {
                    return Err("empty streaming assignment target".to_string());
                }
                let slice = slice
                    .unwrap_or(1)
                    .min(u128::from(width))
                    .try_into()
                    .expect("packed stream width fits u32");
                IrLhs::Stream {
                    parts: ir_parts,
                    width,
                    slice,
                    direction,
                }
            }
        })
    }
}

/// Materialize an explicit cast's target width before the value enters any
/// enclosing assignment context. An unbased unsized fill is contextual only
/// until this boundary (IEEE 1800-2009 §6.24.1); retaining its marker would
/// incorrectly refill a wider destination instead of extending the cast value.
fn ir_to_explicit_cast_storage(
    value: IrExpr,
    width: u32,
    signed: bool,
    two_state: bool,
) -> Result<IrExpr, String> {
    let Some(fill) = value.fill else {
        return ir_to_storage(value, width, signed, two_state);
    };
    let limbs = (width as usize).div_ceil(64);
    let mut materialized = vec![u64::MAX; limbs];
    if let Some(last) = materialized.last_mut() {
        let tail = width % 64;
        if tail != 0 {
            *last = (1u64 << tail) - 1;
        }
    }
    let zeros = vec![0; limbs];
    let (bits, x, z) = match (fill, two_state) {
        (2 | 3, true) => (zeros.clone(), zeros.clone(), zeros),
        (0, _) => (zeros.clone(), zeros.clone(), zeros),
        (1, _) => (materialized, zeros.clone(), zeros),
        (2, _) => (zeros.clone(), materialized, zeros),
        (3, _) => (zeros.clone(), zeros, materialized),
        _ => return Err(format!("invalid explicit-cast fill value {fill}")),
    };
    let constant =
        IrConst::packed(bits, x, z, width, signed, None).map_err(|error| error.to_string())?;
    Ok(IrExpr::new(
        IrExprKind::Const(constant),
        width,
        signed,
        None,
    ))
}

fn parse_decimal_real_literal(token: &str) -> Option<f64> {
    let bytes = token.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'_'
            && (!index
                .checked_sub(1)
                .and_then(|previous| bytes.get(previous))
                .is_some_and(u8::is_ascii_digit)
                || !bytes.get(index + 1).is_some_and(u8::is_ascii_digit))
        {
            return None;
        }
    }
    let normalized = token.replace('_', "");
    let bytes = normalized.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == integer_start {
        return None;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn time_literal_token(source: &str) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut start = 0usize;
    while start < bytes.len() {
        if bytes[start] == b'/' && bytes.get(start + 1) == Some(&b'/') {
            return None;
        }
        if bytes[start] == b'/' && bytes.get(start + 1) == Some(&b'*') {
            start += 2;
            while start + 1 < bytes.len() && !(bytes[start] == b'*' && bytes[start + 1] == b'/') {
                start += 1;
            }
            start = (start + 2).min(bytes.len());
            continue;
        }
        if bytes[start] == b'"' {
            start += 1;
            while start < bytes.len() {
                if bytes[start] == b'\\' {
                    start = (start + 2).min(bytes.len());
                } else if bytes[start] == b'"' {
                    start += 1;
                    break;
                } else {
                    start += 1;
                }
            }
            continue;
        }
        if bytes[start] == b'\\' {
            while start < bytes.len() && !bytes[start].is_ascii_whitespace() {
                start += 1;
            }
            continue;
        }
        if !bytes[start].is_ascii_digit()
            || start > 0
                && matches!(
                    bytes[start - 1],
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$'
                )
        {
            start += 1;
            continue;
        }
        let mut end = start;
        while matches!(bytes.get(end), Some(b'0'..=b'9' | b'_')) {
            end += 1;
        }
        if bytes.get(end) == Some(&b'.') {
            end += 1;
            let fraction = end;
            while matches!(bytes.get(end), Some(b'0'..=b'9' | b'_')) {
                end += 1;
            }
            if !bytes[fraction..end].iter().any(u8::is_ascii_digit) {
                start += 1;
                continue;
            }
        }
        let suffix_end = ["ms", "us", "ns", "ps", "fs", "s"]
            .iter()
            .find_map(|suffix| {
                source[end..]
                    .starts_with(suffix)
                    .then(|| end + suffix.len())
            });
        if let Some(suffix_end) = suffix_end {
            let boundary = bytes.get(suffix_end);
            if !matches!(
                boundary,
                Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$')
            ) {
                return source.get(start..suffix_end);
            }
        }
        start += 1;
    }
    None
}

#[cfg(test)]
mod cast_tests {
    use super::{ir_to_explicit_cast_storage, parse_decimal_real_literal};
    use crate::sim::codegen::lowering::ir_to_storage;
    use crate::sim::ir::{
        IrExpr, IrExprKind, IrLhs, IrModel, IrProcess, IrShape, IrSignal, IrStmt, IrType,
    };
    use crate::sim::opt::{self, OptConfig};

    #[test]
    fn explicit_cast_materializes_unsized_fill_at_target_width() {
        let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
        let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();

        assert_eq!(cast.width(), 1);
        assert_eq!(cast.fill(), None);
        assert!(matches!(
            cast.kind(),
            IrExprKind::Const(value)
                if value.bits() == [1]
                    && value.x_mask() == [0]
                    && value.z_mask() == [0]
        ));
    }

    #[test]
    fn folded_real_comparison_accepts_only_decimal_numeric_tokens() {
        assert_eq!(parse_decimal_real_literal("1.5"), Some(1.5));
        assert_eq!(parse_decimal_real_literal("-2_000e-3"), Some(-2.0));
        assert_eq!(parse_decimal_real_literal("42"), Some(42.0));
        for rejected in ["NaN", "inf", "+inf", "1.", ".5", "1__0", "1e", "8'h1"] {
            assert_eq!(parse_decimal_real_literal(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn narrow_fill_cast_stays_materialized_when_assigned_wider() {
        let build = || {
            let fill = IrExpr::new(IrExprKind::Fill(1), 1, false, Some(1));
            let cast = ir_to_explicit_cast_storage(fill, 1, false, false).unwrap();
            let rhs = ir_to_storage(cast, 8, false, false).unwrap();
            let mut model = IrModel::new("cast".to_string(), 1).unwrap();
            model.signals.push(
                IrSignal::new(
                    "value".to_string(),
                    None,
                    IrType::packed(8, false).unwrap(),
                    None,
                )
                .unwrap(),
            );
            model.processes.push(IrProcess::new(
                "proc".to_string(),
                "cast.initial".to_string(),
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs,
                    nba: false,
                }],
            ));
            model
        };

        for config in [OptConfig::none(), OptConfig::default()] {
            let mut model = build();
            opt::run(&mut model, &config);
            let IrStmt::Assign { rhs, .. } = &model.processes[0].body[0] else {
                panic!("optimizer replaced the cast assignment")
            };
            assert_eq!(rhs.fill(), None);
            if let IrExprKind::Const(value) = rhs.kind() {
                assert_eq!(value.width(), 8);
                assert_eq!(value.bits(), [1]);
                assert_eq!(value.fill(), None);
            }
        }
    }
}

#[cfg(test)]
mod time_literal_source_tests {
    use super::time_literal_token;

    #[test]
    fn source_scanner_ignores_non_token_text() {
        assert_eq!(time_literal_token("8'd1 /* 2ns */ + 8'd2"), None);
        assert_eq!(time_literal_token("8'd1 + 8'd2 // 2ns"), None);
        assert_eq!(time_literal_token(r#""2ns""#), None);
        assert_eq!(time_literal_token(r"\2ns + 1"), None);
        assert_eq!(time_literal_token("design2ns + 1"), None);
        assert_eq!(time_literal_token("design22ns + _12ns"), None);
    }

    #[test]
    fn source_scanner_finds_standalone_time_tokens() {
        assert_eq!(time_literal_token("2.1ns"), Some("2.1ns"));
        assert_eq!(time_literal_token("(2ns + 1ns)"), Some("2ns"));
        assert_eq!(time_literal_token("3 + 40ps"), Some("40ps"));
    }
}
