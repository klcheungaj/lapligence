//! External input.

use super::*;

impl<'a> Codegen<'a> {
    fn packed_plusarg_text(constant: &IrConst, context: &str) -> Result<String, String> {
        if constant.real.is_some() || constant.width == 0 {
            return Err(format!(
                "{context} requires a literal or integral string argument"
            ));
        }
        let byte_count = constant.width.div_ceil(8) as usize;
        let mut bytes = Vec::with_capacity(byte_count);
        for byte in (0..byte_count).rev() {
            let bit = byte * 8;
            let limb = bit / 64;
            let shift = bit % 64;
            let mut value = constant.bits.get(limb).copied().unwrap_or(0) >> shift;
            if shift > 56 {
                value |= constant.bits.get(limb + 1).copied().unwrap_or(0) << (64 - shift);
            }
            bytes.push(value as u8);
        }
        if constant.x.iter().any(|bits| *bits != 0) || constant.z.iter().any(|bits| *bits != 0) {
            return Err(format!(
                "{context} contains unknown bits and cannot be used as a format"
            ));
        }
        let first = bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(bytes.len());
        String::from_utf8(bytes[first..].to_vec()).map_err(|_| {
            format!("{context} must contain valid UTF-8; arbitrary bytes are not supported")
        })
    }

    fn plusarg_string_expr(value: IrStringExpr, context: &str) -> Result<IrPlusArgText, String> {
        if let IrStringExpr::Literal(bytes) = value {
            let text = String::from_utf8(bytes).map_err(|_| {
                format!("{context} must contain valid UTF-8; arbitrary bytes are not supported")
            })?;
            if text.contains('\0') {
                return Err(format!("{context} contains NUL"));
            }
            Ok(IrPlusArgText::Literal(text))
        } else {
            Ok(IrPlusArgText::Dynamic(value))
        }
    }

    pub(super) fn lower_plusarg_text(
        &mut self,
        scope_path: &str,
        node: NodeId,
        context: &str,
    ) -> Result<IrPlusArgText, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::String,
                value,
                ..
            }) => {
                let text = decoded_string_text(value, context)?;
                if text.contains('\0') {
                    return Err(format!("{context} contains NUL"));
                }
                Ok(IrPlusArgText::Literal(text))
            }
            _ if self.is_string_expr(scope_path, node) => {
                Self::plusarg_string_expr(self.lower_string(scope_path, node)?, context)
            }
            _ => {
                let expression = self.lower_expr(scope_path, node)?;
                if let IrExprKind::Const(constant) = &expression.kind {
                    return Ok(IrPlusArgText::Literal(Self::packed_plusarg_text(
                        constant, context,
                    )?));
                }
                if expression.is_real() {
                    return Err(format!("{context} requires a string or integral argument"));
                }
                Ok(IrPlusArgText::Dynamic(IrStringExpr::FromPacked(Box::new(
                    expression,
                ))))
            }
        }
    }

    fn lower_plusarg_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrPlusArgTarget, String> {
        if self.is_string_expr(scope_path, node) {
            self.ensure_string_actual_writable(scope_path, node)?;
            return Ok(IrPlusArgTarget::String {
                address: self.lower_string_actual_address(scope_path, node)?,
            });
        }
        if self.is_chandle_expr(scope_path, node) {
            return Err(format!(
                "$value$plusargs destination must be packed, real, or string storage in `{scope_path}`"
            ));
        }
        let lhs = self.lower_lhs(scope_path, node)?;
        if let IrLhs::Ref {
            const_ref: true, ..
        } = &lhs
        {
            return Err(format!(
                "$value$plusargs destination cannot be a const ref in `{scope_path}`"
            ));
        }
        let real = match &lhs {
            IrLhs::Whole(index) => match self.model.signal(*index).ty {
                IrType::Real { shortreal } => Some(shortreal),
                IrType::Packed { .. } => None,
            },
            IrLhs::WholeRef {
                width: 0,
                shortreal,
                ..
            } => Some(*shortreal),
            IrLhs::ArrayElem {
                arr,
                elem_sel: IrElemSel::Whole,
                ..
            } => {
                let arr = self.reference_array(*arr);
                self.model
                    .array(arr)
                    .real
                    .then_some(self.model.array(arr).shortreal)
            }
            _ => None,
        };
        if let Some(shortreal) = real {
            return Ok(IrPlusArgTarget::Real {
                lhs: Box::new(lhs),
                shortreal,
            });
        }
        let IrType::Packed {
            width,
            signed,
            two_state,
        } = self.reference_lhs_type(&lhs).ok_or_else(|| {
            format!("$value$plusargs destination is not writable storage in `{scope_path}`")
        })?
        else {
            return Err(format!(
                "$value$plusargs destination is not a supported packed lvalue in `{scope_path}`"
            ));
        };
        if width == 0 {
            return Err(format!(
                "$value$plusargs destination has no resolved type in `{scope_path}`"
            ));
        }
        Ok(IrPlusArgTarget::Packed {
            lhs: Box::new(lhs),
            width,
            signed,
            two_state,
        })
    }

    pub(super) fn lower_file_input_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrFileInputTarget, String> {
        match self.lower_plusarg_target(scope_path, node)? {
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => Ok(IrFileInputTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            }),
            IrPlusArgTarget::Real { lhs, shortreal } => {
                Ok(IrFileInputTarget::Real { lhs, shortreal })
            }
            IrPlusArgTarget::String { address } => Ok(IrFileInputTarget::String { address }),
        }
    }

    pub(super) fn lower_file_read_target(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<IrFileReadTarget, String> {
        if let Some(array) = self.array_of(node).cloned() {
            let array = self.reference_array(array.ir);
            if self.model.array(array).real {
                return Err(format!(
                    "$fread destination array must contain packed elements in `{scope_path}`"
                ));
            }
            return Ok(IrFileReadTarget::Array { array });
        }
        match self.lower_plusarg_target(scope_path, node)? {
            IrPlusArgTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            } => Ok(IrFileReadTarget::Packed {
                lhs,
                width,
                signed,
                two_state,
            }),
            IrPlusArgTarget::Real { .. } | IrPlusArgTarget::String { .. } => Err(format!(
                "$fread destination must be a packed value or unpacked array in `{scope_path}`"
            )),
        }
    }

    pub(super) fn file_input_actual(&self, node: NodeId) -> Result<NodeId, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !operands.is_empty() => Ok(operands[0]),
            _ => Ok(node),
        }
    }

    pub(in super::super) fn lower_plusarg_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args = self.node(call).children.clone();
        match name {
            "$test$plusargs" => {
                let [pattern] = args.as_slice() else {
                    return Err(format!(
                        "$test$plusargs requires exactly one argument in `{scope_path}`"
                    ));
                };
                let pattern =
                    self.lower_plusarg_text(scope_path, *pattern, "$test$plusargs pattern")?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::TestPlusArgs { pattern }),
                    32,
                    true,
                    None,
                ))
            }
            "$value$plusargs" => {
                let [format_node, destination] = args.as_slice() else {
                    return Err(format!(
                        "$value$plusargs requires exactly two arguments in `{scope_path}`"
                    ));
                };
                let format =
                    self.lower_plusarg_text(scope_path, *format_node, "$value$plusargs format")?;
                if let IrPlusArgText::Literal(format) = &format {
                    validate_plusarg_format(format, scope_path)?;
                }
                let destination = match self.kind(*destination) {
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::Assignment,
                        operands,
                        ..
                    }) if !operands.is_empty() => operands[0],
                    _ => *destination,
                };
                let target = self.lower_plusarg_target(scope_path, destination)?;
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::ValuePlusArgs { format, target }),
                    32,
                    true,
                    None,
                ))
            }
            _ => Err(format!("unsupported plusarg system function {name}")),
        }
    }

    /// Lower the optional command argument of `$system` into an owned string
    /// expression. `None` preserves the standard's omitted-argument
    /// `system(NULL)` query, while an explicit empty argument remains an owned
    /// empty string. Slang has already checked the system-call arity; retaining
    /// the check here keeps malformed owned IR from reaching the emitter.
    pub(in super::super) fn lower_system_command(
        &mut self,
        scope_path: &str,
        args: &[NodeId],
    ) -> Result<Option<IrStringExpr>, String> {
        match args {
            [] => Ok(None),
            [arg] => self.lower_string(scope_path, *arg).map(Some),
            _ => Err(format!(
                "$system accepts at most one string argument in `{scope_path}`"
            )),
        }
    }
}
