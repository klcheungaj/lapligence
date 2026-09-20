//! Fixed-array reductions execute in the caller's lexical evaluation frame.

use super::*;
use crate::sim::ir::{IrFixedArrayReduction, IrFixedArrayReductionSource};

impl Codegen<'_> {
    pub(in super::super) fn lower_fixed_array_reduction(
        &mut self,
        path: &str,
        call: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let (name, receiver) = match self.kind(call) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.as_str(), *receiver),
            _ => return Ok(None),
        };
        let operation = match name {
            "sum" => IrContainerReduction::Sum,
            "product" => IrContainerReduction::Product,
            "and" => IrContainerReduction::BitAnd,
            "or" => IrContainerReduction::BitOr,
            "xor" => IrContainerReduction::BitXor,
            _ => return Ok(None),
        };
        let Some(descriptor) = self.query_descriptor(receiver).cloned() else {
            return Ok(None);
        };
        if !matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
            return Ok(None);
        }
        let (left, right, element) = reduction_element(&descriptor)?;
        let element_width = Self::fixed_descriptor_width(&element).ok_or_else(|| {
            format!("fixed-array reduction in `{path}` requires a supported fixed integral element")
        })?;
        let element_signed = element.info.signed;
        let with_node = self.container_method_with_node(path, call, receiver)?;
        if with_node.is_none() {
            if !self.container_method_arguments(path, call, receiver)?.is_empty() {
                return Err(format!("fixed-array reduction in `{path}` has unexpected arguments"));
            }
            if !reduction_integral(&element) {
                return Err(format!(
                    "fixed-array reduction in `{path}` needs an integral element or an integral with expression"
                ));
            }
        }
        let count = i64::from(left).abs_diff(i64::from(right)) + 1;
        // Do not flatten an ordinary memory into a giant concatenation merely
        // to reduce it. Value receivers still capture once before iteration.
        // array_of also unwraps casts and assignments for lvalue discovery.
        // Those are value expressions here and must not bypass evaluation.
        let direct = if matches!(
            self.kind(receiver),
            NodeKind::Array { .. }
                | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        ) {
            self.array_of(receiver)
                .filter(|array| {
                    !array.real && array.dims.len() == 1 && array.elem_width == element_width
                })
                .map(|array| self.reference_array(array.ir))
        } else {
            None
        };
        let source = if let Some(array) = direct {
            if self.model.arrays[array].total != count {
                return Err(format!("fixed-array reduction storage extent mismatch in `{path}`"));
            }
            IrFixedArrayReductionSource::Array(array)
        } else {
            let width = Self::fixed_descriptor_width(&descriptor).ok_or_else(|| {
                format!("fixed-array reduction receiver payload exceeds supported width in `{path}`")
            })?;
            let value = self.lower_expr(path, receiver)?;
            if value.width != width || value.is_real() {
                return Err(format!("fixed-array reduction receiver width mismatch in `{path}`"));
            }
            IrFixedArrayReductionSource::Value(Box::new(value))
        };
        let item_name = self.new_fn_name(path, "reduction_item");
        let index_name = self.new_fn_name(path, "reduction_index");
        let value = if let Some(with_node) = with_node {
            let iterator = self.db.method_call_iterator(call).ok_or_else(|| {
                format!("fixed-array reduction in `{path}` has no captured iterator binding")
            })?;
            let saved = self.fixed_method_iterators.insert(
                iterator,
                FixedMethodIterator {
                    descriptor: element.clone(),
                    item_name: item_name.clone(),
                    index_name: index_name.clone(),
                },
            );
            let lowered = self.lower_expr(path, with_node);
            if let Some(saved) = saved {
                self.fixed_method_iterators.insert(iterator, saved);
            } else {
                self.fixed_method_iterators.remove(&iterator);
            }
            let value = lowered?;
            if value.is_real() || value.width == 0 {
                return Err(format!(
                    "fixed-array reduction with expression in `{path}` must be integral"
                ));
            }
            let width = value.width;
            let signed = value.signed;
            ir_to_storage(value, width, signed, self.db.is_two_state_type(with_node))?
        } else {
            IrExpr::new(
                IrExprKind::LocalRead(item_name.clone()),
                element_width,
                element_signed,
                None,
            )
        };
        let width = value.width;
        let signed = value.signed;
        Ok(Some(IrExpr::new(
            IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
                source,
                operation,
                left,
                right,
                element_width,
                element_signed,
                element_two_state: element.two_state,
                item_name,
                index_name,
                value,
            })),
            width,
            signed,
            None,
        )))
    }

    pub(in super::super) fn fixed_reduction_index(
        &self,
        path: &str,
        args: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let Some(receiver) = args.first() else {
            return Ok(None);
        };
        let NodeKind::Expr(ExprKind::Ref { target: Some(target) }) = self.kind(*receiver) else {
            return Ok(None);
        };
        let Some(iterator) = self.fixed_method_iterators.get(target) else {
            return Ok(None);
        };
        if args.len() > 2
            || args
                .get(1)
                .is_some_and(|dimension| self.eval_bound_i128(*dimension).ok() != Some(1))
        {
            return Err(format!(
                "fixed-array iterator index in `{path}` supports only the current dimension (constant 1)"
            ));
        }
        Ok(Some(IrExpr::new(
            IrExprKind::LocalRead(iterator.index_name.clone()),
            32,
            true,
            None,
        )))
    }
}

fn reduction_integral(descriptor: &TypeDescriptor) -> bool {
    matches!(&descriptor.shape, TypeShape::PackedAtom { .. })
        || matches!(&descriptor.shape, TypeShape::Aggregate(layout)
            if matches!(layout.kind, AggregateKind::PackedStruct | AggregateKind::PackedUnion))
}

/// Peel only the outer dimension; a row is one iterator value, not a flattened
/// list of packed leaves. This is essential to nested reductions with `with`.
fn reduction_element(descriptor: &TypeDescriptor) -> Result<(i32, i32, TypeDescriptor), String> {
    let TypeShape::FixedArray { dimensions, element } = &descriptor.shape else {
        return Err("reduction receiver is not a fixed unpacked array".to_owned());
    };
    let &(left, right) = dimensions
        .first()
        .ok_or("fixed-array reduction has no dimension")?;
    let mut immediate = if dimensions.len() == 1 {
        element.as_ref().clone()
    } else {
        TypeDescriptor {
            shape: TypeShape::FixedArray {
                dimensions: dimensions[1..].to_vec(),
                element: element.clone(),
            },
            ..descriptor.clone()
        }
    };
    if !reduction_integral(&immediate) {
        immediate.info.signed = false;
    }
    Ok((left, right, immediate))
}

#[cfg(test)]
mod tests;
