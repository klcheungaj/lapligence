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

    /// A context-determined packed operator: its operands take the common type.
    fn lower_context_binary(
        &self,
        scope_path: &str,
        op: IrBinOp,
        operands: &[NodeId],
        a: IrExpr,
        b: IrExpr,
    ) -> Result<IrExpr, String> {
        let (width, signed) = (a.width.max(b.width), a.signed && b.signed);
        let context = "arithmetic/bitwise context";
        let a = self.context_operand(scope_path, operands[0], a, width, signed, context)?;
        let b = self.context_operand(scope_path, operands[1], b, width, signed, context)?;
        Ok(common_bin_expr(op, a, b))
    }

    /// Propagate the common type of an operation into `value`, lowered from its
    /// operand `node`. An operand that propagated this very type to its own
    /// operands keeps it; otherwise a chain of operators would visit each
    /// operand once per operator around it.
    pub(super) fn context_operand(
        &self,
        scope_path: &str,
        node: NodeId,
        value: IrExpr,
        width: u32,
        signed: bool,
        context: &str,
    ) -> Result<IrExpr, String> {
        let propagated = (value.width, value.signed) == (width, signed)
            && match (self.kind(node), &value.kind) {
                (
                    NodeKind::Expr(ExprKind::Operation {
                        op:
                            Operation::Add
                            | Operation::Subtract
                            | Operation::Multiply
                            | Operation::Divide
                            | Operation::Modulo
                            | Operation::BitwiseAnd
                            | Operation::BitwiseOr
                            | Operation::BitwiseXor
                            | Operation::BitwiseXnor,
                        assignment: false,
                        ..
                    }),
                    IrExprKind::Bin { op, .. },
                ) => is_context_binary(*op),
                (
                    NodeKind::Expr(
                        ExprKind::Conditional { .. }
                        | ExprKind::Operation {
                            op: Operation::Conditional,
                            ..
                        },
                    ),
                    IrExprKind::Mux { .. },
                ) => true,
                _ => false,
            };
        if propagated {
            return Ok(value);
        }
        checked_operand_with_context(value, width, signed, scope_path, context)
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
        if let Some(value) = self.lower_special_operation(scope_path, otype, operands)? {
            return Ok(value);
        }
        // Operators nest, so the arms hold no operand: a frame per operator
        // of a chain stays on the stack (`crate::stack`).
        match otype {
            Operation::Add
            | Operation::Subtract
            | Operation::Multiply
            | Operation::Divide
            | Operation::Modulo
            | Operation::Power
            | Operation::BitwiseAnd
            | Operation::BitwiseOr
            | Operation::BitwiseXor
            | Operation::BitwiseXnor
            | Operation::LogicalAnd
            | Operation::LogicalOr
            | Operation::Imply
            | Operation::LogicalEquivalence
            | Operation::Equal
            | Operation::NotEqual
            | Operation::CaseEqual
            | Operation::CaseNotEqual
            | Operation::WildEqual
            | Operation::WildNotEqual
            | Operation::Less
            | Operation::LessEqual
            | Operation::Greater
            | Operation::GreaterEqual
            | Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight => {
                let a = self.lower_expr(scope_path, operands[0])?;
                let b = self.lower_expr(scope_path, operands[1])?;
                self.binary_operation(scope_path, otype, operands, a, b)
            }
            Operation::Conditional => self.lower_conditional(scope_path, operands),
            Operation::UnaryMinus
            | Operation::UnaryPlus
            | Operation::BitwiseNot
            | Operation::ReductionAnd
            | Operation::ReductionNand
            | Operation::ReductionOr
            | Operation::ReductionNor
            | Operation::ReductionXor
            | Operation::ReductionXnor => {
                let a = self.lower_expr(scope_path, operands[0])?;
                self.unary_operation(scope_path, otype, operands[0], a)
            }
            Operation::LogicalNot => self.lower_logical_not(scope_path, operands[0]),
            Operation::Inside => self.lower_inside(scope_path, operands),
            Operation::Concat => self.lower_concatenation(scope_path, reordered, operands),
            Operation::MultiConcat => self.lower_replication(scope_path, operands),
            Operation::Cast => Err(format!(
                "cast expressions are not supported in `{scope_path}` \
                 (the database does not capture the cast typespec)"
            )),
            // Only the selected operand is evaluated, so the unselected
            // ones contribute neither side effects nor sensitivity.
            Operation::MinTypMax => {
                self.lower_expr(scope_path, operands[self.min_typ_max_operand()])
            }
            other => Err(format!(
                "unsupported operation op type {other:?} in `{scope_path}`"
            )),
        }
    }

    /// The operations that read their operands as whole aggregate, container
    /// or native values rather than as packed ones; `None` for the others.
    fn lower_special_operation(
        &mut self,
        scope_path: &str,
        otype: Operation,
        operands: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        if let Some(value) = self.lower_native_comparison(scope_path, otype, operands)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.lower_record_view_equality(scope_path, otype, operands)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.lower_real_array_equality(scope_path, otype, operands)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.lower_container_equality(scope_path, otype, operands)? {
            return Ok(Some(value));
        }
        if matches!(
            otype,
            Operation::Equal | Operation::NotEqual | Operation::CaseEqual | Operation::CaseNotEqual
        ) {
            // Column-layout records compare column by column.
            if self.record_columns
                && operands.iter().any(|node| {
                    self.column_record_type(*node) || self.column_record_storage(*node).is_some()
                })
            {
                if let Some(value) =
                    self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
                {
                    return Ok(Some(value));
                }
            }
            let oversized = operands.iter().any(|node| {
                self.query_descriptor(*node)
                    .and_then(Self::fixed_descriptor_width_bits)
                    .is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
            });
            // Casts that reshape or clear X/Z, conditionals and calls compare
            // as descriptor values. Mid-size descriptor storage takes the same
            // path rather than flattening every cell into one packed operand.
            let converts = operands
                .iter()
                .any(|node| self.converting_descriptor_cast(*node));
            // A whole tagged-union member array is checked against its tag
            // as a descriptor value operand.
            let guarded = self.record_columns
                && operands
                    .iter()
                    .any(|node| matches!(self.record_member_guard(*node), Ok(Some(_))));
            let arrays = !guarded
                && self.array_of(operands[0]).is_some()
                && self.array_of(operands[1]).is_some();
            if (!arrays || converts)
                && (oversized || operands.iter().all(|node| self.descriptor_operand(*node)))
            {
                return Ok(Some(IrExpr::new(
                    IrExprKind::FixedValueCompare {
                        left: Box::new(self.lower_fixed_value(scope_path, operands[0])?),
                        right: Box::new(self.lower_fixed_value(scope_path, operands[1])?),
                        case: matches!(otype, Operation::CaseEqual | Operation::CaseNotEqual),
                        negate: matches!(otype, Operation::NotEqual | Operation::CaseNotEqual),
                    },
                    1,
                    false,
                    None,
                )));
            }

            if let (Some(left), Some(right), false) = (
                self.array_of(operands[0]),
                self.array_of(operands[1]),
                converts,
            ) {
                if self.model.arrays[left.ir].sparse() && self.model.arrays[right.ir].sparse() {
                    return Ok(Some(IrExpr::new(
                        IrExprKind::FixedArrayCompare {
                            left: left.ir,
                            right: right.ir,
                            case: matches!(otype, Operation::CaseEqual | Operation::CaseNotEqual),
                            negate: matches!(otype, Operation::NotEqual | Operation::CaseNotEqual),
                        },
                        1,
                        false,
                        None,
                    )));
                }
            }
            if let Some(value) =
                self.lower_unpacked_aggregate_comparison(scope_path, otype, operands)?
            {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// A binary operation of two lowered operands.
    fn binary_operation(
        &mut self,
        scope_path: &str,
        otype: Operation,
        operands: &[NodeId],
        a: IrExpr,
        b: IrExpr,
    ) -> Result<IrExpr, String> {
        let maxw = |a: &IrExpr, b: &IrExpr| a.width.max(b.width);
        match otype {
            Operation::Add => {
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Add, a, b));
                }
                self.lower_context_binary(scope_path, IrBinOp::Add, operands, a, b)
            }
            Operation::Subtract => {
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                self.lower_context_binary(scope_path, IrBinOp::Sub, operands, a, b)
            }
            Operation::Multiply => {
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                self.lower_context_binary(scope_path, IrBinOp::Mul, operands, a, b)
            }
            Operation::Divide | Operation::Modulo | Operation::Power => {
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
                    self.lower_context_binary(scope_path, f, operands, a, b)
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
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                self.lower_context_binary(scope_path, IrBinOp::BitAnd, operands, a, b)
            }
            Operation::BitwiseOr => {
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                self.lower_context_binary(scope_path, IrBinOp::BitOr, operands, a, b)
            }
            Operation::BitwiseXor => {
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                self.lower_context_binary(scope_path, IrBinOp::BitXor, operands, a, b)
            }
            Operation::BitwiseXnor => {
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                self.lower_context_binary(scope_path, IrBinOp::BitXNor, operands, a, b)
            }
            Operation::LogicalAnd => Ok(cmp_expr_ir(IrBinOp::LogAnd, a, b)),
            Operation::LogicalOr => Ok(cmp_expr_ir(IrBinOp::LogOr, a, b)),
            // This is the ordinary Boolean `->` expression.  Property
            // implication (`|->`/`|=>`) has separate operation tags and never
            // reaches this expression lowering path.
            Operation::Imply => Ok(cmp_expr_ir(IrBinOp::LogImpl, a, b)),
            Operation::LogicalEquivalence => Ok(cmp_expr_ir(IrBinOp::LogEquiv, a, b)),
            Operation::Equal => common_cmp_expr_ir(IrBinOp::Eq, a, b, scope_path),
            Operation::NotEqual => common_cmp_expr_ir(IrBinOp::Neq, a, b, scope_path),
            Operation::CaseEqual => {
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on a real operand in `{scope_path}` is illegal (SV 11.3.1, Table 11-1)"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseEq, a, b, scope_path)
            }
            Operation::CaseNotEqual => {
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on a real operand in `{scope_path}` is illegal (SV 11.3.1, Table 11-1)"
                    ));
                }
                common_cmp_expr_ir(IrBinOp::CaseNeq, a, b, scope_path)
            }
            Operation::WildEqual | Operation::WildNotEqual => {
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
            Operation::Less => common_cmp_expr_ir(IrBinOp::Lt, a, b, scope_path),
            Operation::LessEqual => common_cmp_expr_ir(IrBinOp::Le, a, b, scope_path),
            Operation::Greater => common_cmp_expr_ir(IrBinOp::Gt, a, b, scope_path),
            Operation::GreaterEqual => common_cmp_expr_ir(IrBinOp::Ge, a, b, scope_path),
            Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight => {
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
            other => Err(format!(
                "unsupported operation op type {other:?} in `{scope_path}`"
            )),
        }
    }

    /// A unary operation of the lowered operand `a` of `operand`.
    fn unary_operation(
        &mut self,
        scope_path: &str,
        otype: Operation,
        operand: NodeId,
        a: IrExpr,
    ) -> Result<IrExpr, String> {
        match otype {
            Operation::UnaryMinus => {
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
                    self.kind(operand),
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
            Operation::UnaryPlus => Ok(a),
            Operation::BitwiseNot => {
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
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedAnd, a))
            }
            Operation::ReductionNand => {
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNand, a))
            }
            Operation::ReductionOr => {
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedOr, a))
            }
            Operation::ReductionNor => {
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNor, a))
            }
            Operation::ReductionXor => {
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXor, a))
            }
            Operation::ReductionXnor => {
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXNor, a))
            }
            other => Err(format!(
                "unsupported operation op type {other:?} in `{scope_path}`"
            )),
        }
    }

    fn lower_logical_not(&mut self, scope_path: &str, operand: NodeId) -> Result<IrExpr, String> {
        // Handles (events, chandles, mailboxes) test against null.
        let a = self.lower_boolean_expr(scope_path, operand)?;
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

    fn lower_inside(&mut self, scope_path: &str, operands: &[NodeId]) -> Result<IrExpr, String> {
        let Some((value_node, item_nodes)) = operands.split_first() else {
            return Err(format!("empty inside expression in `{scope_path}`"));
        };
        if item_nodes.is_empty() {
            return Err(format!("inside set is empty in `{scope_path}`"));
        }
        if self.is_handle_set_operand(scope_path, *value_node) {
            return self.with_set_arrays(|this| {
                this.lower_handle_inside(scope_path, *value_node, item_nodes)
            });
        }
        // A member with an effect at its place: compare in member order
        // (decision EO-D2).
        if item_nodes
            .iter()
            .any(|item| self.in_place_member_effect(*item))
        {
            return self.lower_inside_in_order(scope_path, *value_node, item_nodes);
        }
        if self.is_string_expr(scope_path, *value_node) {
            return self.with_set_arrays(|this| {
                let value = this.lower_string(scope_path, *value_node)?;
                let items = this.lower_inside_string_items(scope_path, item_nodes)?;
                Ok(object_query(
                    IrObjectQuery::StringInside { value, items },
                    1,
                    false,
                ))
            });
        }
        self.with_set_arrays(|this| {
            let value = this.lower_expr(scope_path, *value_node)?;
            let items = this.lower_inside_items(scope_path, item_nodes)?;
            Ok(IrExpr::new(
                IrExprKind::Inside {
                    value: Box::new(value),
                    items,
                },
                1,
                false,
                None,
            ))
        })
    }

    fn lower_concatenation(
        &mut self,
        scope_path: &str,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        let mut parts = Vec::new();
        for operand in operands {
            // A zero replication has size zero and is ignored
            // (11.4.12.1).
            if !self.is_zero_replication(*operand) {
                parts.push(self.lower_expr(scope_path, *operand)?);
            }
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

    fn lower_replication(
        &mut self,
        scope_path: &str,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        let count = {
            let value = self.eval_bits(operands[0])?;
            if value.is_unknown() {
                return Err(format!("unknown replication count in `{scope_path}`"));
            }
            value
                .to_u128()
                .and_then(|count| u64::try_from(count).ok())
                .ok_or_else(|| format!("replication count does not fit in u64 in `{scope_path}`"))?
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

    /// Lower a compound assignment or increment/decrement to the canonical
    /// mutation expression. The target is resolved once by the mutation
    /// emitter; the caller may keep the yielded value or discard it in
    /// statement position.
    pub(in super::super) fn lower_mutation_expression(
        &mut self,
        scope_path: &str,
        op: Operation,
        operands: &[NodeId],
        _assignment: bool,
    ) -> Result<IrExpr, String> {
        let lhs_node = *operands.first().ok_or_else(|| {
            format!("assignment-like expression in `{scope_path}` has no left operand")
        })?;
        let overloaded = matches!(
            op,
            Operation::OverloadUpdate | Operation::OverloadPostUpdate
        );
        if overloaded {
            self.check_overloaded_update_target(scope_path, lhs_node)?;
        }
        if !overloaded {
            if let Some(select) = self.native_member_select(scope_path, lhs_node)? {
                let post = matches!(op, Operation::PostIncrement | Operation::PostDecrement);
                return self.native_member_select_mutation(
                    scope_path,
                    &select,
                    post,
                    &mut |codegen, current| {
                        codegen.mutation_value(scope_path, op, operands, current)
                    },
                );
            }
        }
        let lhs = self.lower_lhs(scope_path, lhs_node)?;
        // An overload's target is an ordinary typed lvalue (an unpacked
        // record's members form a `Stream` of leaves), never a streaming
        // concatenation: those have no type an overload could match.
        if matches!(lhs, IrLhs::Stream { .. }) && !overloaded {
            return Err(format!(
                "assignment-like expression to a streaming target in `{scope_path}` is not supported"
            ));
        }
        // Lowering the LHS expression here is only for its final type. The
        // runtime read is reconstructed from the canonical descriptor, so a
        // dynamic index is never evaluated by both the read and the write.
        let current_type = self.lower_expr(scope_path, lhs_node)?;
        let post = matches!(
            op,
            Operation::PostIncrement | Operation::PostDecrement | Operation::OverloadPostUpdate
        );
        let reads_current = !matches!(op, Operation::Assignment);
        let value = if overloaded {
            // IEEE 1800-2009 11.11: the bound function's call reads the target
            // through the mutation's captured current value, so the target's
            // selectors run once for the read and the write.
            let call = *operands.get(1).ok_or_else(|| {
                format!("overloaded update in `{scope_path}` has no function call")
            })?;
            self.overload_current
                .push(super::super::OverloadCurrent::Packed(
                    current_type.width,
                    current_type.signed,
                ));
            let value = self.lower_expr(scope_path, call);
            self.overload_current.pop();
            apply_lhs_assignment_context(&self.model, &lhs, value?)
        } else if op == Operation::Assignment {
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
            self.mutation_value(scope_path, op, operands, current)?
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

    /// The value a built-in assignment-like operation stores, from the
    /// value `current` of its target.
    fn mutation_value(
        &mut self,
        scope_path: &str,
        op: Operation,
        operands: &[NodeId],
        current: IrExpr,
    ) -> Result<IrExpr, String> {
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
        super::super::lower_compound_expr_ir(scope_path, arithmetic, current, rhs)
    }

    /// An overloaded update (IEEE 1800-2009 11.11) that yields a value or binds
    /// its target once runs as one packed mutation. Targets above the packed
    /// value limit and native records have no such form yet: their statement
    /// forms and assignment values with side-effect-free targets re-read the
    /// target (`lower_native_overload_update_value`) instead.
    pub(in super::super) fn check_overloaded_update_target(
        &self,
        scope_path: &str,
        target: NodeId,
    ) -> Result<(), String> {
        let Some(descriptor) = self.query_descriptor(target) else {
            return Ok(());
        };
        if !matches!(
            descriptor.shape,
            TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
        ) {
            return Ok(());
        }
        let limited = match Self::fixed_descriptor_width_bits(descriptor) {
            Some(width) => width > u64::from(LLG_MAX_WIDTH),
            None => true,
        };
        if limited {
            return Err(format!(
                "an overloaded operator update on a target above the {LLG_MAX_WIDTH}-bit packed value limit or with native members yields a value only as the right-hand side of an assignment, needs side-effect-free target selectors, and has a postfix value only for native records, in `{scope_path}`"
            ));
        }
        Ok(())
    }

    /// The target value an overloaded update's function call reads
    /// (`OverloadCurrent`): the enclosing mutation expression's capture.
    pub(in super::super) fn overload_current_read(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Option<Result<IrExpr, String>> {
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::OverloadCurrent,
                ..
            })
        ) {
            return None;
        }
        Some(match self.overload_current.last() {
            Some(&super::super::OverloadCurrent::Packed(width, signed)) => Ok(IrExpr::new(
                IrExprKind::LocalRead("_llg_mut_current".to_owned()),
                width,
                signed,
                None,
            )),
            Some(&super::super::OverloadCurrent::Target(target)) => {
                self.lower_expr(scope_path, target)
            }
            None => Err(format!(
                "overloaded operator target value outside its update in `{scope_path}`"
            )),
        })
    }

    /// The node an operand stands for: an `OverloadCurrent` operand of an
    /// update whose target is read again (`OverloadCurrent::Target`) is that
    /// target; every other node is itself.
    pub(in super::super) fn overload_operand(&self, node: NodeId) -> NodeId {
        let unwrapped = self.p30_unwrap_cast(node);
        match (self.kind(unwrapped), self.overload_current.last()) {
            (
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::OverloadCurrent,
                    ..
                }),
                Some(&super::super::OverloadCurrent::Target(target)),
            ) => target,
            _ => node,
        }
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
