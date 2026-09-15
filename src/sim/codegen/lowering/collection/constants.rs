//! Constants.

use super::*;

impl<'a> Codegen<'a> {

    // ── Constant-ish bound evaluation ──────────────────────────────────────

    /// Evaluate a constant expression node (part-select bound) to an integer.
    pub(in super::super) fn eval_bound_i128(&self, node: NodeId) -> Result<i128, String> {
        match self.eval_bits(node) {
            Ok(v) if !v.is_unknown() => if v.signed {
                v.to_i128()
            } else {
                v.to_u128().and_then(|value| value.try_into().ok())
            }
            .ok_or_else(|| "part_select bound does not fit in i128".to_string()),
            Ok(_) => Err("unknown part_select bound".to_string()),
            Err(e) => Err(format!("part_select bound: {e}")),
        }
    }

    /// Evaluate a constant-ish expression node to a 4-state value, mirroring
    /// `core::elab::Resolver::eval_expr` for the constructs that can appear in
    /// elaborated bound positions.
    pub(in super::super) fn eval_bits(&self, node: NodeId) -> Result<elab::Value, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                let mut value = val_from_value_data(value, *size)?;
                if self.signed_based_constant(node) {
                    if let Val::Bits(bits) = &mut value {
                        if let Some(width) = self
                            .signed_based_literal_info(node)
                            .1
                            .map(|width| width as usize)
                        {
                            if width < bits.width() {
                                *bits = bits.resize(width, true);
                            }
                        }
                        bits.signed = true;
                    }
                }
                match value {
                    Val::Bits(b) => Ok(b),
                    Val::Str(value) => string_to_value(&value),
                    Val::Real(_) => Err("non-integer constant in bound".to_string()),
                }
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(b)) => Ok(b.clone()),
                _ => Err("enum constant without value in bound".to_string()),
            },
            NodeKind::Expr(ExprKind::Ref { target }) => {
                match target.and_then(|t| self.param_vals.get(&t).map(|value| (t, value))) {
                    Some((_, Val::Bits(b))) => Ok(b.clone()),
                    Some((target, Val::Str(value))) => match self.kind(target) {
                        NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                            Some(width) => {
                                Ok(string_to_value(value)?.cast(width as usize, ty.signed))
                            }
                            None => Err("non-integer parameter in bound".to_string()),
                        },
                        _ => Err("non-integer parameter in bound".to_string()),
                    },
                    Some((_, Val::Real(_))) => Err("non-integer parameter in bound".to_string()),
                    None => Err("unresolved reference in bound".to_string()),
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                operands,
                ..
            }) => self.eval_operation_bits(*op, *reordered, operands),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
                if !is_real_kind(&ty.kind) && ty.kind != "string" && !is_handle_kind(&ty.kind) =>
            {
                match self.eval_decl_value(node)? {
                    Val::Bits(value) => Ok(value),
                    _ => Err("non-integral cast in bound".to_owned()),
                }
            }
            NodeKind::SysCall { name }
                if matches!(
                    name.as_str(),
                    "$countones" | "$onehot" | "$onehot0" | "$isunknown"
                ) =>
            {
                let [arg] = self.node(node).children.as_slice() else {
                    return Err(format!("{name} requires exactly one argument"));
                };
                let arg = self.eval_bits(*arg)?;
                Ok(match name.as_str() {
                    "$countones" => elab::countones(&arg),
                    "$onehot" => elab::onehot(&arg),
                    "$onehot0" => elab::onehot0(&arg),
                    _ => elab::isunknown(&arg),
                })
            }
            other => Err(format!("unsupported bound expression: {other:?}")),
        }
    }

    pub(super) fn collected_parameter_value(
        &self,
        scope: NodeId,
        parameter: NodeId,
        frontend_value: Option<&Val>,
    ) -> Result<Option<Val>, String> {
        let fallback = || frontend_value.map(materialize_parameter_value);
        let NodeKind::Param { ty, .. } = self.kind(parameter) else {
            return Ok(fallback());
        };
        if self.db.parameter_is_overridden(parameter) {
            return Ok(fallback());
        }
        let parameter_name = self.node(parameter).name.as_str();
        let assignment_rhs = self.node(scope).children.iter().find_map(|child| {
            if !matches!(self.kind(*child), NodeKind::ParamAssign { .. }) {
                return None;
            }
            let [lhs, rhs] = self.node(*child).children.as_slice() else {
                return None;
            };
            (self.node(*lhs).name == parameter_name).then_some(*rhs)
        });
        let assignment_rhs = assignment_rhs.or_else(|| {
            self.node(parameter)
                .children
                .iter()
                .copied()
                .find(|child| matches!(self.kind(*child), NodeKind::Expr(_)))
        });
        let Some(assignment_rhs) = assignment_rhs else {
            return Ok(fallback());
        };
        let integral_cast = matches!(
            self.kind(assignment_rhs),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
            if !is_real_kind(&ty.kind) && ty.kind != "string" && !is_handle_kind(&ty.kind)
        );
        if frontend_value.is_some()
            && !integral_cast
            && !self.contains_time_literal(assignment_rhs, &mut HashSet::new())
        {
            return Ok(fallback());
        }
        let Ok(value) = self.eval_decl_value(assignment_rhs) else {
            return Ok(fallback());
        };
        if ty.kind == "real" {
            return Ok(Some(Val::Real(match value {
                Val::Bits(value) => value.to_real(),
                Val::Real(value) => value,
                Val::Str(_) => return Ok(fallback()),
            })));
        }
        if ty.kind == "shortreal" {
            let value = match value {
                Val::Bits(value) => value.to_real(),
                Val::Real(value) => value,
                Val::Str(_) => return Ok(fallback()),
            };
            return Ok(Some(Val::Real((value as f32) as f64)));
        }
        let Some(width) = ty.width else {
            return Ok(fallback());
        };
        let value = match value {
            Val::Bits(value) => value,
            Val::Real(value) => elab::real_to_bits(value, width as usize, ty.signed),
            Val::Str(_) => return Ok(fallback()),
        };
        Ok(Some(Val::Bits(materialize_decl_cast_value(
            value,
            width as usize,
            ty.signed,
            self.db.is_two_state_type(parameter) || is_two_state_kind(&ty.kind),
        ))))
    }

    fn contains_time_literal(&self, node: NodeId, visited: &mut HashSet<NodeId>) -> bool {
        if !visited.insert(node) {
            return false;
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { const_type, .. }) => {
                *const_type == ConstantType::Time
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands
                .iter()
                .any(|operand| self.contains_time_literal(*operand, visited)),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.contains_time_literal(*operand, visited)
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|child| self.contains_time_literal(*child, visited)),
        }
    }

    /// Evaluate the packed/real constants accepted in scalar declaration
    /// initializers.  This stays on the owned database and extends the
    /// integer-only bound evaluator only for conversion system functions.
    pub(in super::super) fn eval_decl_value(&self, node: NodeId) -> Result<Val, String> {
        // Explicit casts are value-materialization boundaries. Handle them
        // before the general integral evaluator, whose Value result retains
        // an unbased fill marker for surrounding expression contexts.
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
                if !is_real_kind(&ty.kind) && ty.kind != "string" && !is_handle_kind(&ty.kind)
        ) {
            if let Ok(bits) = self.eval_bits(node) {
                return Ok(Val::Bits(bits));
            }
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                time_scale,
                ..
            }) => {
                let value = val_from_value_data(value, *size)?;
                if *const_type == ConstantType::Time && self.round_time_literals {
                    let Val::Real(value) = value else {
                        return Err("time literal has no real value".to_owned());
                    };
                    return Ok(Val::Real(self.rounded_time_literal(
                        node,
                        value,
                        source,
                        *time_scale,
                    )?));
                }
                Ok(value)
            }
            NodeKind::Expr(ExprKind::Ref { target }) => target
                .and_then(|target| self.param_vals.get(&target).cloned())
                .ok_or_else(|| "unresolved reference in declaration initializer".to_string()),
            NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => {
                super::super::validate_operation_arity(*op, operands.len(), "constant expression")?;
                if *op == Operation::MinTypMax {
                    return self.eval_decl_value(operands[1]);
                }
                if *op == Operation::Conditional {
                    let condition = self.eval_bits(operands[0])?;
                    let known = condition
                        .to_u128()
                        .ok_or("unknown condition in constant real expression")?;
                    return self.eval_decl_value(operands[if known == 0 { 2 } else { 1 }]);
                }
                let values = operands
                    .iter()
                    .map(|operand| self.eval_decl_value(*operand))
                    .collect::<Result<Vec<_>, _>>()?;
                if !values.iter().any(|value| matches!(value, Val::Real(_))) {
                    return Err("integral constant operation could not be evaluated".to_owned());
                }
                let real = |value: &Val| match value {
                    Val::Real(value) => Ok(*value),
                    Val::Bits(value) => Ok(value.to_real()),
                    Val::Str(_) => Err("string operand in constant real expression".to_owned()),
                };
                let logical = |value: &Val| match value {
                    Val::Real(value) => {
                        Ok(elab::Value::from_u64(u64::from(*value != 0.0), 1, false))
                    }
                    Val::Bits(value) => Ok(value.clone()),
                    Val::Str(_) => Err("string operand in constant logical expression".to_owned()),
                };
                let value = match *op {
                    Operation::UnaryPlus => Val::Real(real(&values[0])?),
                    Operation::UnaryMinus => Val::Real(-real(&values[0])?),
                    Operation::Add => Val::Real(real(&values[0])? + real(&values[1])?),
                    Operation::Subtract => Val::Real(real(&values[0])? - real(&values[1])?),
                    Operation::Multiply => Val::Real(real(&values[0])? * real(&values[1])?),
                    Operation::Divide => Val::Real(real(&values[0])? / real(&values[1])?),
                    Operation::Modulo => Val::Real(real(&values[0])? % real(&values[1])?),
                    Operation::Power => Val::Real(real(&values[0])?.powf(real(&values[1])?)),
                    Operation::LogicalAnd => {
                        Val::Bits(elab::log_and(&logical(&values[0])?, &logical(&values[1])?))
                    }
                    Operation::LogicalOr => {
                        Val::Bits(elab::log_or(&logical(&values[0])?, &logical(&values[1])?))
                    }
                    Operation::Imply => Val::Bits(elab::log_imply(
                        &logical(&values[0])?,
                        &logical(&values[1])?,
                    )),
                    Operation::LogicalEquivalence => Val::Bits(elab::log_equiv(
                        &logical(&values[0])?,
                        &logical(&values[1])?,
                    )),
                    _ => return Err(format!("unsupported constant real operation {op:?}")),
                };
                match value {
                    Val::Bits(value) => Ok(Val::Bits(value)),
                    Val::Real(value) => value
                        .is_finite()
                        .then_some(Val::Real(value))
                        .ok_or_else(|| "constant real expression is not finite".to_owned()),
                    Val::Str(_) => Err("constant logical expression produced a string".to_owned()),
                }
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if is_real_kind(&ty.kind) => {
                let value = match self.eval_decl_value(*operand)? {
                    Val::Bits(value) => value.to_real(),
                    Val::Real(value) => value,
                    Val::Str(_) => {
                        return Err(
                            "string-to-real cast in declaration initializer is not supported"
                                .to_owned(),
                        )
                    }
                };
                Ok(Val::Real(if ty.kind == "shortreal" {
                    (value as f32) as f64
                } else {
                    value
                }))
            }
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
                propagated,
            }) if !is_real_kind(&ty.kind) && ty.kind != "string" && !is_handle_kind(&ty.kind) => {
                if !cast_kind_known {
                    return Err("declaration-initializer cast kind cannot be determined".to_owned());
                }
                let width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or(ty.width)
                    .ok_or("integral declaration-initializer cast has no width")?;
                if width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "declaration-initializer cast is {width} bits wide; maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                let cast_bits = |mut value: elab::Value| {
                    if *propagated {
                        value.signed = ty.signed;
                    }
                    let signed = if *size_cast { value.signed } else { ty.signed };
                    Val::Bits(materialize_decl_cast_value(
                        value,
                        width as usize,
                        signed,
                        *two_state || is_two_state_kind(&ty.kind),
                    ))
                };
                match self.eval_decl_value(*operand)? {
                    Val::Bits(value) => Ok(cast_bits(value)),
                    Val::Str(value) => Ok(cast_bits(string_to_value(&value)?)),
                    Val::Real(value) => Ok(cast_bits(elab::real_to_bits(
                        value,
                        width as usize,
                        if *size_cast { false } else { ty.signed },
                    ))),
                }
            }
            NodeKind::SysCall { name }
                if matches!(
                    name.as_str(),
                    "$rtoi"
                        | "$itor"
                        | "$realtobits"
                        | "$bitstoreal"
                        | "$shortrealtobits"
                        | "$bitstoshortreal"
                ) =>
            {
                let [arg] = self.node(node).children.as_slice() else {
                    return Err(format!("{name} requires exactly one argument"));
                };
                let arg = self.eval_decl_value(*arg)?;
                match (name.as_str(), arg) {
                    ("$rtoi", Val::Real(value)) => Ok(Val::Bits(elab::rtoi_value(value))),
                    ("$rtoi", Val::Bits(value)) => Ok(Val::Bits(elab::rtoi_value(value.to_real()))),
                    ("$itor", Val::Bits(value)) => Ok(Val::Real(value.to_real())),
                    ("$itor", Val::Real(value)) => Ok(Val::Real(value)),
                    ("$realtobits", Val::Real(value)) => {
                        Ok(Val::Bits(elab::real_to_ieee_bits(value)))
                    }
                    ("$realtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(elab::real_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoreal", Val::Bits(value)) if value.width() == 64 => Ok(Val::Real(
                        elab::ieee_bits_to_real(&value)
                            .ok_or_else(|| "invalid $bitstoreal width".to_string())?,
                    )),
                    ("$shortrealtobits", Val::Real(value)) => {
                        Ok(Val::Bits(elab::shortreal_to_ieee_bits(value)))
                    }
                    ("$shortrealtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(elab::shortreal_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoshortreal", Val::Bits(value)) if value.width() == 32 => Ok(Val::Real(
                        elab::ieee_bits_to_shortreal(&value)
                            .ok_or_else(|| "invalid $bitstoshortreal width".to_string())?,
                    )),
                    _ => Err(format!(
                        "invalid argument to {name} in declaration initializer"
                    )),
                }
            }
            other => Err(format!(
                "unsupported declaration initializer expression: {other:?}"
            )),
        }
    }

    fn eval_operation_bits(
        &self,
        op: Operation,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<elab::Value, String> {
        super::super::validate_operation_arity(op, operands.len(), "constant expression")?;
        let u = |i: usize| self.eval_bits(operands[i]);
        macro_rules! b {
            ($i:expr) => {
                u($i)?
            };
        }
        match op {
            Operation::UnaryMinus => Ok(elab::minus(&b!(0))),
            Operation::UnaryPlus => Ok(b!(0)),
            Operation::LogicalNot => Ok(elab::log_not(&b!(0))),
            Operation::BitwiseNot => Ok(elab::bit_neg(&b!(0))),
            Operation::ReductionAnd => Ok(elab::unary_and(&b!(0))),
            Operation::ReductionNand => Ok(elab::unary_nand(&b!(0))),
            Operation::ReductionOr => Ok(elab::unary_or(&b!(0))),
            Operation::ReductionNor => Ok(elab::unary_nor(&b!(0))),
            Operation::ReductionXor => Ok(elab::unary_xor(&b!(0))),
            Operation::ReductionXnor => Ok(elab::unary_xnor(&b!(0))),
            Operation::Subtract => Ok(elab::sub(&b!(0), &b!(1))),
            Operation::Divide => Ok(elab::div(&b!(0), &b!(1))),
            Operation::Modulo => Ok(elab::rem(&b!(0), &b!(1))),
            Operation::Equal => Ok(elab::eq(&b!(0), &b!(1))),
            Operation::NotEqual => Ok(elab::neq(&b!(0), &b!(1))),
            Operation::CaseEqual => Ok(elab::case_eq(&b!(0), &b!(1))),
            Operation::CaseNotEqual => Ok(elab::case_neq(&b!(0), &b!(1))),
            Operation::WildEqual => Ok(elab::wildcard_eq(&b!(0), &b!(1))),
            Operation::WildNotEqual => Ok(elab::wildcard_neq(&b!(0), &b!(1))),
            Operation::Greater => Ok(elab::gt(&b!(0), &b!(1))),
            Operation::GreaterEqual => Ok(elab::ge(&b!(0), &b!(1))),
            Operation::Less => Ok(elab::lt(&b!(0), &b!(1))),
            Operation::LessEqual => Ok(elab::le(&b!(0), &b!(1))),
            Operation::ShiftLeft => Ok(elab::shl(&b!(0), &b!(1))),
            Operation::ShiftRight => Ok(elab::shr(&b!(0), &b!(1))),
            Operation::ArithmeticShiftLeft => Ok(elab::arith_shl(&b!(0), &b!(1))),
            Operation::ArithmeticShiftRight => Ok(elab::arith_shr(&b!(0), &b!(1))),
            Operation::Add => Ok(elab::add(&b!(0), &b!(1))),
            Operation::Multiply => Ok(elab::mul(&b!(0), &b!(1))),
            Operation::Power => Ok(elab::power(&b!(0), &b!(1))),
            Operation::LogicalAnd => Ok(elab::log_and(&b!(0), &b!(1))),
            Operation::LogicalOr => Ok(elab::log_or(&b!(0), &b!(1))),
            Operation::Imply => {
                let left = b!(0);
                // Match Slang's short-circuit constant evaluation: a known
                // false antecedent determines the result without touching
                // the consequent.
                if left.to_u128() == Some(0) {
                    Ok(elab::Value::from_u64(1, 1, false))
                } else {
                    let right = b!(1);
                    Ok(elab::log_imply(&left, &right))
                }
            }
            Operation::LogicalEquivalence => Ok(elab::log_equiv(&b!(0), &b!(1))),
            Operation::BitwiseAnd => Ok(elab::bit_and(&b!(0), &b!(1))),
            Operation::BitwiseOr => Ok(elab::bit_or(&b!(0), &b!(1))),
            Operation::BitwiseXor => Ok(elab::bit_xor(&b!(0), &b!(1))),
            Operation::BitwiseXnor => Ok(elab::bit_xnor(&b!(0), &b!(1))),
            Operation::Conditional => Ok(elab::cond(&b!(0), &b!(1), &b!(2))),
            Operation::MinTypMax => Ok(b!(0)),
            Operation::Concat => {
                let mut parts = Vec::with_capacity(operands.len());
                for i in 0..operands.len() {
                    parts.push(b!(i));
                }
                if reordered {
                    parts.reverse();
                }
                Ok(elab::concat(&parts))
            }
            Operation::MultiConcat => {
                let count = b!(0);
                if count.is_unknown() {
                    return Err("unknown replication count".to_string());
                }
                let n: usize = count
                    .to_u128()
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| "replication count does not fit in usize".to_string())?;
                let mut parts = Vec::with_capacity(operands.len().saturating_sub(1));
                for i in 1..operands.len() {
                    parts.push(b!(i));
                }
                let pat = elab::concat(&parts);
                let total_width = pat
                    .width()
                    .checked_mul(n)
                    .ok_or_else(|| "replication width overflow".to_string())?;
                if total_width > LLG_MAX_WIDTH as usize {
                    return Err(format!(
                        "replication result is too wide ({total_width} bits; max {LLG_MAX_WIDTH})"
                    ));
                }
                let mut bits = Vec::with_capacity(total_width);
                for _ in 0..n {
                    bits.extend(pat.bits.iter().cloned());
                }
                Ok(elab::Value::from_bits(bits, false))
            }
            other => Err(format!("unsupported operation op type {other:?} in bound")),
        }
    }
}
