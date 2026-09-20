//! Conditional values retain array element boundaries across payload flattening.
use super::*;

impl Codegen<'_> {
    pub(super) fn lower_conditional(
        &mut self,
        scope_path: &str,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        // Arity is checked by lower_operation. Slang has already reconciled
        // the alternatives' types; query the typed operand before flattening.
        let array = self
            .query_descriptor(operands[1])
            .filter(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
            .map(array_merge_default)
            .transpose()?;
        let sel = self.lower_boolean_expr(scope_path, operands[0])?;
        self.lower_conditional_arms(scope_path, sel, operands[1], operands[2], array)
    }

    pub(in super::super) fn lower_conditional_predicate(
        &mut self,
        scope_path: &str,
        predicate: &crate::core::db::ConditionalPredicate,
    ) -> Result<IrExpr, String> {
        if predicate.clauses.is_empty() {
            return Err(format!("conditional predicate has no clauses in `{scope_path}`"));
        }
        if predicate.has_patterns() {
            return Err(format!(
                "conditional predicate pattern matching (`matches`) is not supported in `{scope_path}`"
            ));
        }
        if let [clause] = predicate.clauses.as_slice() {
            return self.lower_boolean_expr(scope_path, clause.expression);
        }
        let clauses = predicate
            .clauses
            .iter()
            .map(|clause| self.lower_boolean_expr(scope_path, clause.expression))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(IrExpr::new(IrExprKind::Predicate { clauses }, 1, false, None))
    }

    pub(in super::super) fn lower_predicate_conditional(
        &mut self,
        scope_path: &str,
        predicate: &crate::core::db::ConditionalPredicate,
        if_true: NodeId,
        if_false: NodeId,
    ) -> Result<IrExpr, String> {
        let array = self
            .query_descriptor(if_true)
            .filter(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
            .map(array_merge_default)
            .transpose()?;
        let sel = self.lower_conditional_predicate(scope_path, predicate)?;
        self.lower_conditional_arms(scope_path, sel, if_true, if_false, array)
    }

    fn lower_conditional_arms(
        &mut self,
        scope_path: &str,
        sel: IrExpr,
        if_true: NodeId,
        if_false: NodeId,
        array: Option<(u32, IrConst)>,
    ) -> Result<IrExpr, String> {
        let a = self.lower_expr(scope_path, if_true)?;
        let b = self.lower_expr(scope_path, if_false)?;
        if let Some((width, element_default)) = array {
            if a.width != width || b.width != width || a.is_real() || b.is_real() {
                return Err(format!(
                    "fixed-array conditional payload width mismatch in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::ArrayMux {
                    sel: Box::new(sel),
                    a: Box::new(IrExpr::resize_to(a, width, false)),
                    b: Box::new(IrExpr::resize_to(b, width, false)),
                    element_default: Box::new(element_default),
                },
                width,
                false,
                None,
            ));
        }
        let (width, signed) = if a.is_real() || b.is_real() {
            (REAL_EXPR_WIDTH, true)
        } else {
            (a.width.max(b.width), a.signed && b.signed)
        };
        let (a, b) = if width == REAL_EXPR_WIDTH {
            (a, b)
        } else {
            (
                checked_operand_with_context(a, width, signed, scope_path, "conditional context")?,
                checked_operand_with_context(b, width, signed, scope_path, "conditional context")?,
            )
        };
        Ok(IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(sel),
                a: Box::new(a),
                b: Box::new(b),
            },
            width,
            signed,
            None,
        ))
    }
}

fn array_merge_default(descriptor: &TypeDescriptor) -> Result<(u32, IrConst), String> {
    let TypeShape::FixedArray {
        dimensions,
        element,
    } = &descriptor.shape
    else {
        return Err("conditional merge requires a fixed unpacked array".to_owned());
    };
    if dimensions.is_empty() {
        return Err("conditional array has no unpacked dimension".to_owned());
    }
    let width = Codegen::fixed_descriptor_width(descriptor)
        .ok_or("conditional array requires a supported fixed integral payload")?;
    // TypeDescriptor coalesces consecutive unpacked dimensions. Only peel
    // the outermost dimension: a differing row defaults the *whole* row.
    let immediate = if dimensions.len() == 1 {
        *element.clone()
    } else {
        TypeDescriptor {
            shape: TypeShape::FixedArray {
                dimensions: dimensions[1..].to_vec(),
                element: element.clone(),
            },
            ..descriptor.clone()
        }
    };
    let default = Codegen::fixed_descriptor_uninitialized(&immediate)
        .ok_or("conditional array element has no supported default-uninitialized payload")?;
    if default.width == 0 || !width.is_multiple_of(default.width) {
        return Err("conditional array element width does not divide its payload".to_owned());
    }
    Ok((width, default))
}

#[cfg(test)]
mod tests;
