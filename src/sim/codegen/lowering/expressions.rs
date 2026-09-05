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

    /// Lower `h` to IR and render it in one step.  This is THE expression
    /// seam: every consumer sees exactly the pre-IR rendered C text.
    pub(super) fn emit_expr(&mut self, scope_path: &str, h: NodeId) -> Result<String, String> {
        let ir = self.lower_expr(scope_path, h)?;
        self.render_ir_code(&ir)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    pub(super) fn lower_expr(&mut self, scope_path: &str, h: NodeId) -> Result<IrExpr, String> {
        match self.kind(h) {
            NodeKind::Expr(ExprKind::Constant { .. }) => {
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
                let width = match self.const_of_node(*width_expr) {
                    Ok(c) if c.real.is_none() => c.width,
                    Ok(_) => {
                        return Err(format!(
                            "indexed part-select width cannot be real in `{scope_path}`"
                        ))
                    }
                    Err(_) => {
                        return Err(format!(
                            "indexed part-select width must be a constant in `{scope_path}`"
                        ))
                    }
                };
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
            NodeKind::Expr(ExprKind::Cast { operand, ty }) => {
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
                let (w, s) = match (ty.width, ty.signed) {
                    (Some(w), s) => (w, s),
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
                         runtime supports at most {LLG_MAX_WIDTH}"
                    ));
                }
                // Value-preserving conversion (LRM 1800-2009 §6.24.1: the
                // cast yields the value a variable of the cast type holds
                // after the assignment — extension follows the SOURCE's
                // signedness, so int'(8'hFF) is 255, not -1).
                Ok(ir_to_vector(v, w, s)?)
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
                // 2-part interface member access (`m.data`): a read of the
                // resolved per-port copy var.
                if let Some(info) = self.hier_path_signal(h) {
                    return Ok(sig_read_expr_full(info));
                }
                Err(format!(
                    "hierarchical references are not supported (in `{scope_path}`)"
                ))
            }
            other => Err(format!(
                "unsupported expression in `{scope_path}` (node kind {other:?})"
            )),
        }
    }

    fn lower_ref_expr(
        &self,
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
                if let Some((cname, w, s)) = f.locals.get(&t) {
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
                for (node, (cname, w, s)) in &f.locals {
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
                let mut c = read_const_from(value, *size)?;
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
                Ok(common_bin_expr(IrBinOp::Add, a, b))
            }
            vpiSubOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                Ok(common_bin_expr(IrBinOp::Sub, a, b))
            }
            vpiMultOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                Ok(common_bin_expr(IrBinOp::Mul, a, b))
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
                // The runtime returns all-X for div/mod/pow with operands
                // wider than 64 bits; reject before emission instead.
                if a.width > 64 || b.width > 64 {
                    return Err(format!(
                        "wide division/modulo/power not yet supported (operand \
                         wider than 64 bits) in `{scope_path}`"
                    ));
                }
                let f = match otype {
                    vpiDivOp => IrBinOp::Div,
                    vpiModOp => IrBinOp::Mod,
                    _ => IrBinOp::Pow,
                };
                if matches!(f, IrBinOp::Div | IrBinOp::Mod) {
                    Ok(common_bin_expr(f, a, b))
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
                Ok(common_bin_expr(IrBinOp::BitAnd, a, b))
            }
            vpiBitOrOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitOr, a, b))
            }
            vpiBitXorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitXor, a, b))
            }
            vpiBitXNorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitXNor, a, b))
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
                Ok(cmp_expr_ir(IrBinOp::Eq, a, b))
            }
            vpiNeqOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Neq, a, b))
            }
            vpiCaseEqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(cmp_expr_ir(IrBinOp::CaseEq, a, b))
            }
            vpiCaseNeqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(cmp_expr_ir(IrBinOp::CaseNeq, a, b))
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
                Ok(cmp_expr_ir(IrBinOp::Lt, a, b))
            }
            vpiLeOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Le, a, b))
            }
            vpiGtOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Gt, a, b))
            }
            vpiGeOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Ge, a, b))
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
                         the v1 runtime supports at most {LLG_MAX_WIDTH}"
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
                    let c = self.const_of_node(operands[0])?;
                    if c.x.iter().any(|&v| v != 0) || c.z.iter().any(|&v| v != 0) {
                        return Err(format!("unknown replication count in `{scope_path}`"));
                    }
                    c.bits.first().copied().unwrap_or(0)
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
                         the v1 runtime supports at most {LLG_MAX_WIDTH}"
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
        self.lhs_to_ir(&lh)
    }

    /// Convert a pre-IR [`Lhs`] to its [`IrLhs`] form using the registered
    /// model indices.
    fn lhs_to_ir(&self, lh: &Lhs) -> Result<IrLhs, String> {
        Ok(match lh {
            Lhs::Whole(info) => IrLhs::Whole(info.ir),
            Lhs::WholeRef {
                addr,
                width,
                signed,
            } => IrLhs::WholeRef {
                addr: addr.clone(),
                width: *width,
                signed: *signed,
            },
            Lhs::Bit(info, ie) => IrLhs::Bit(info.ir, verbatim_code(ie)),
            Lhs::Part(info, left, right) => {
                let (left, right, _) =
                    checked_select_bounds(*left, *right, "assignment part select")?;
                IrLhs::Part(info.ir, left, right)
            }
            Lhs::IdxPart(info, be, we, neg) => {
                IrLhs::IdxPart(info.ir, verbatim_code(be), verbatim_code(we), *neg != 0)
            }
            Lhs::ArrayElem(ae) => IrLhs::ArrayElem {
                arr: ae.arr.ir,
                indices: ae.index_codes.iter().map(|c| verbatim_code(c)).collect(),
                elem_sel: match &ae.elem_sel {
                    ElemSel::Whole => IrElemSel::Whole,
                    ElemSel::Part(l, r) => {
                        let (left, right, _) =
                            checked_select_bounds(*l, *r, "array-element assignment part select")?;
                        IrElemSel::Part(left, right)
                    }
                    ElemSel::Bit(s) => IrElemSel::Bit(Box::new(verbatim_code(s))),
                },
            },
        })
    }
}
