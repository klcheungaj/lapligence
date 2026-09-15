//! Operations.

use super::*;

impl<'a> Codegen<'a> {
    /// Lower one operation, mirroring the pre-IR emitter's operand shapes,
    /// result widths/signedness and error strings arm-for-arm.
    pub(super) fn lower_logical_chain(
        &mut self,
        scope_path: &str,
        operation: Operation,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        let mut pending = operands.iter().rev().copied().collect::<Vec<_>>();
        let mut values = Vec::new();
        while let Some(node) = pending.pop() {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Operation {
                    op,
                    reordered: false,
                    assignment: false,
                    operands,
                }) if *op == operation && operands.len() == 2 => {
                    pending.push(operands[1]);
                    pending.push(operands[0]);
                }
                _ => values.push(self.lower_expr(scope_path, node)?),
            }
        }
        let mut values = values.into_iter();
        let first = values
            .next()
            .ok_or_else(|| format!("logical operation has no operands in `{scope_path}`"))?;
        let ir_operation = if operation == Operation::LogicalAnd {
            IrBinOp::LogAnd
        } else {
            IrBinOp::LogOr
        };
        Ok(values.fold(first, |left, right| cmp_expr_ir(ir_operation, left, right)))
    }

    pub(super) fn lower_operation(
        &mut self,
        scope_path: &str,
        otype: Operation,
        reordered: bool,
        assignment: bool,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        super::super::validate_operation_arity(otype, operands.len(), scope_path)?;
        if assignment
            || matches!(
                otype,
                Operation::PreIncrement
                    | Operation::PreDecrement
                    | Operation::PostIncrement
                    | Operation::PostDecrement
            )
        {
            return self.lower_mutation_expression(scope_path, otype, operands, assignment);
        }
        macro_rules! op {
            ($i:expr) => {
                self.lower_expr(scope_path, operands[$i])?
            };
        }
        let maxw = |a: &IrExpr, b: &IrExpr| a.width.max(b.width);

        match otype {
            Operation::Add => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Add, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Add, a, b, scope_path)
            }
            Operation::Subtract => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Sub, a, b, scope_path)
            }
            Operation::Multiply => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                common_bin_expr_with_context(IrBinOp::Mul, a, b, scope_path)
            }
            Operation::Divide | Operation::Modulo | Operation::Power => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    let rop = match otype {
                        Operation::Divide => IrRealBinOp::Div,
                        Operation::Modulo => IrRealBinOp::Mod,
                        _ => IrRealBinOp::Pow,
                    };
                    return Ok(real_bin_expr(rop, a, b));
                }
                let f = match otype {
                    Operation::Divide => IrBinOp::Div,
                    Operation::Modulo => IrBinOp::Mod,
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
            Operation::BitwiseAnd => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitAnd, a, b, scope_path)
            }
            Operation::BitwiseOr => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitOr, a, b, scope_path)
            }
            Operation::BitwiseXor => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXor, a, b, scope_path)
            }
            Operation::BitwiseXnor => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                common_bin_expr_with_context(IrBinOp::BitXNor, a, b, scope_path)
            }
            Operation::LogicalAnd => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogAnd, a, b))
            }
            Operation::LogicalOr => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogOr, a, b))
            }
            // This is the ordinary Boolean `->` expression.  Property
            // implication (`|->`/`|=>`) has separate operation tags and never
            // reaches this expression lowering path.
            Operation::Imply => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogImpl, a, b))
            }
            Operation::LogicalEquivalence => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogEquiv, a, b))
            }
            Operation::Equal => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Eq, a, b, scope_path)
            }
            Operation::NotEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Neq, a, b, scope_path)
            }
            Operation::CaseEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseEq, a, b, scope_path)
            }
            Operation::CaseNotEqual => {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(value);
                }
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseNeq, a, b, scope_path)
            }
            Operation::WildEqual | Operation::WildNotEqual => {
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
                let op = if otype == Operation::WildEqual {
                    IrBinOp::WildEq
                } else {
                    IrBinOp::WildNeq
                };
                Ok(cmp_expr_ir(op, a, b))
            }
            Operation::Less => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Lt, a, b, scope_path)
            }
            Operation::LessEqual => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Le, a, b, scope_path)
            }
            Operation::Greater => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Gt, a, b, scope_path)
            }
            Operation::GreaterEqual => {
                let a = op!(0);
                let b = op!(1);
                common_cmp_expr_ir(IrBinOp::Ge, a, b, scope_path)
            }
            Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "shift on real value in `{scope_path}` is not supported"
                    ));
                }
                let f = match otype {
                    Operation::ShiftLeft => IrBinOp::Shl,
                    Operation::ShiftRight => IrBinOp::Shr,
                    Operation::ArithmeticShiftLeft => IrBinOp::Ashl,
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
            Operation::Conditional => {
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
            Operation::UnaryMinus => {
                let a = op!(0);
                let w = a.width;
                if a.is_real() {
                    return Ok(real_un_expr(a));
                }
                // Unary minus of an unsized decimal literal (`-3`): a normalized
                // literal can be an unsigned 64-bit constant,
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
            Operation::UnaryPlus => {
                let a = op!(0);
                Ok(a)
            }
            Operation::LogicalNot => {
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
            Operation::BitwiseNot => {
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
            Operation::ReductionAnd => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedAnd, a))
            }
            Operation::ReductionNand => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNand, a))
            }
            Operation::ReductionOr => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedOr, a))
            }
            Operation::ReductionNor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNor, a))
            }
            Operation::ReductionXor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXor, a))
            }
            Operation::ReductionXnor => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXNor, a))
            }
            Operation::Inside => {
                let Some((value_node, item_nodes)) = operands.split_first() else {
                    return Err(format!("empty inside expression in `{scope_path}`"));
                };
                if item_nodes.is_empty() {
                    return Err(format!("inside set is empty in `{scope_path}`"));
                }
                if self.is_chandle_expr(scope_path, *value_node) {
                    return Err(format!(
                        "chandle-valued inside selector is not supported in `{scope_path}`"
                    ));
                }
                if self.is_string_expr(scope_path, *value_node) {
                    let value = self.lower_string(scope_path, *value_node)?;
                    let items = self.lower_inside_string_items(scope_path, item_nodes)?;
                    return Ok(object_query(
                        IrObjectQuery::StringInside { value, items },
                        1,
                        false,
                    ));
                }
                let value = self.lower_expr(scope_path, *value_node)?;
                let items = self.lower_inside_items(scope_path, item_nodes)?;
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
            Operation::Concat => {
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
                         the runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Concat { parts },
                    width,
                    false,
                    None,
                ))
            }
            Operation::MultiConcat => {
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
                         the runtime maximum supported width is {LLG_MAX_WIDTH}"
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
            Operation::Cast => Err(format!(
                "cast expressions are not supported in `{scope_path}` \
                 (the database does not capture the cast typespec)"
            )),
            Operation::MinTypMax => {
                let a = op!(0);
                Ok(a)
            }
            other => Err(format!(
                "unsupported operation op type {other:?} in `{scope_path}`"
            )),
        }
    }

    fn lower_mutation_expression(
        &mut self,
        scope_path: &str,
        op: Operation,
        operands: &[NodeId],
        _assignment: bool,
    ) -> Result<IrExpr, String> {
        let lhs_node = *operands.first().ok_or_else(|| {
            format!("assignment-like expression in `{scope_path}` has no left operand")
        })?;
        let lhs = self.lower_lhs(scope_path, lhs_node)?;
        if matches!(lhs, IrLhs::Stream { .. }) {
            return Err(format!(
                "assignment-like expression to a streaming target in `{scope_path}` is not supported"
            ));
        }
        // Lowering the LHS expression here is only for its final type. The
        // runtime read is reconstructed from the canonical descriptor, so a
        // dynamic index is never evaluated by both the read and the write.
        let current_type = self.lower_expr(scope_path, lhs_node)?;
        let post = matches!(op, Operation::PostIncrement | Operation::PostDecrement);
        let reads_current = !matches!(op, Operation::Assignment);
        let value = if op == Operation::Assignment {
            let rhs_node = *operands.get(1).ok_or_else(|| {
                format!("assignment expression in `{scope_path}` has no right operand")
            })?;
            let rhs = self.lower_expr(scope_path, rhs_node)?;
            apply_lhs_assignment_context(&self.model, &lhs, rhs)
        } else {
            let current = IrExpr::new(
                IrExprKind::LocalRead("_llg_mut_current".to_owned()),
                current_type.width,
                current_type.signed,
                None,
            );
            let rhs = if matches!(
                op,
                Operation::PreIncrement
                    | Operation::PostIncrement
                    | Operation::PreDecrement
                    | Operation::PostDecrement
            ) {
                if current.is_real() {
                    real_literal_expr(1.0)
                } else {
                    IrExpr::new(
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
                    )
                }
            } else {
                let rhs_node = *operands.get(1).ok_or_else(|| {
                    format!("compound assignment in `{scope_path}` has no right operand")
                })?;
                self.lower_expr(scope_path, rhs_node)?
            };
            let arithmetic = if matches!(op, Operation::PreDecrement | Operation::PostDecrement) {
                Operation::Subtract
            } else if matches!(op, Operation::PreIncrement | Operation::PostIncrement) {
                Operation::Add
            } else {
                op
            };
            super::super::lower_compound_expr_ir(scope_path, arithmetic, current, rhs)?
        };
        Ok(IrExpr::new(
            IrExprKind::Mutation(Box::new(crate::sim::ir::IrMutationExpr {
                lhs,
                value: Box::new(value),
                current_width: current_type.width,
                current_signed: current_type.signed,
                reads_current,
                post,
            })),
            current_type.width,
            current_type.signed,
            None,
        ))
    }

    pub(super) fn lower_member_select_index(
        &mut self,
        scope_path: &str,
        base: NodeId,
        index: NodeId,
    ) -> Result<IrExpr, String> {
        if let Some((_, member)) = self.packed_member_info(base) {
            let canonical = matches!(member.packed_ranges.as_slice(), [range]
                if range.right == 0 && range.left == i128::from(member.width) - 1);
            if !canonical {
                let relative = self.aggregate_member_relative_bound(
                    &member.name,
                    &member.packed_ranges,
                    self.eval_bound_i128(index)?,
                )?;
                return Ok(lhs_integer_expr(i128::from(relative)));
            }
        }
        self.lower_packed_index(scope_path, base, index)
    }
}
