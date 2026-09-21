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
        let structure = self
            .query_descriptor(operands[1])
            .filter(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct,
                        ..
                    })
                )
            })
            .map(structure_merge_members)
            .transpose()?;
        let sel = self.lower_boolean_expr(scope_path, operands[0])?;
        self.lower_conditional_arms(
            scope_path,
            sel,
            operands[1],
            operands[2],
            array,
            structure,
        )
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
        let structure = self
            .query_descriptor(if_true)
            .filter(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct,
                        ..
                    })
                )
            })
            .map(structure_merge_members)
            .transpose()?;
        let sel = self.lower_conditional_predicate(scope_path, predicate)?;
        self.lower_conditional_arms(
            scope_path,
            sel,
            if_true,
            if_false,
            array,
            structure,
        )
    }

    fn lower_conditional_arms(
        &mut self,
        scope_path: &str,
        sel: IrExpr,
        if_true: NodeId,
        if_false: NodeId,
        array: Option<(u32, IrConst)>,
        structure: Option<Vec<IrConditionalMember>>,
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
        if let Some(members) = structure {
            let width = members
                .iter()
                .try_fold(0u32, |total, member| {
                    total.checked_add(member.width)
                })
                .ok_or_else(|| {
                    format!("fixed-structure conditional payload width overflow in `{scope_path}`")
                })?;
            if width == 0
                || a.width != width
                || b.width != width
                || a.is_real()
                || b.is_real()
            {
                return Err(format!(
                    "fixed-structure conditional payload width mismatch in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::StructMux {
                    sel: Box::new(sel),
                    a: Box::new(IrExpr::resize_to(a, width, false)),
                    b: Box::new(IrExpr::resize_to(b, width, false)),
                    members,
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

fn structure_merge_members(
    descriptor: &TypeDescriptor,
) -> Result<Vec<IrConditionalMember>, String> {
    let TypeShape::Aggregate(layout) = &descriptor.shape else {
        return Err("conditional merge requires an unpacked structure".to_owned());
    };
    if layout.kind != AggregateKind::UnpackedStruct || layout.members.is_empty() {
        return Err("conditional merge requires a nonempty unpacked structure".to_owned());
    }

    // The flattened representation places the first declared member at the
    // most-significant end, matching fixed aggregate projections and defaults.
    let mut offset = 0u32;
    let mut members = Vec::with_capacity(layout.members.len());
    for member in layout.members.iter().rev() {
        let width = Codegen::fixed_descriptor_width(&member.descriptor)
            .ok_or("conditional structure member has no supported fixed payload")?;
        let default = Codegen::fixed_descriptor_uninitialized(&member.descriptor)
            .ok_or("conditional structure member has no supported default-uninitialized payload")?;
        if width == 0 || default.width != width {
            return Err("conditional structure member default width mismatch".to_owned());
        }
        members.push(IrConditionalMember {
            offset,
            width,
            default,
        });
        offset = offset
            .checked_add(width)
            .ok_or("conditional structure payload width overflow")?;
    }
    if offset != Codegen::fixed_descriptor_width(descriptor).unwrap_or(0) {
        return Err("conditional structure payload width disagrees with members".to_owned());
    }
    Ok(members)
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
