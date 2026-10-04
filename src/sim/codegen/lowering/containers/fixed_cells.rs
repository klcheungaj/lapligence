//! Cell-wise receivers for fixed-array methods and membership.
//!
//! A receiver that names stored cells (a whole array or a selection of its
//! leading unpacked dimensions) is traversed in place by a runtime loop.
//! Neither the generated code nor the IR grows with the element count, and
//! descriptor-backed storage is never flattened into one packed value.

use super::*;
use crate::sim::ir::{IrFixedArrayCells, IrFixedArrayOrder, IrFixedArrayOrderMethod};

/// Largest dense selection still traversed by per-element straight-line IR.
/// Above it, and for every descriptor-backed array, the cell-wise loop wins:
/// an unrolled sort costs n(n-1)/2 comparisons and unrolled code grows with n.
pub(in crate::sim::codegen) const FIXED_CELL_UNROLL_LIMIT: u64 = 16;

impl Codegen<'_> {
    /// Resolve a receiver to stored cells without evaluating it. Casts and
    /// other value expressions are not storage and return `None`. Selectors
    /// are lowered as expressions; the cell-wise operation evaluates them once
    /// before touching any cell.
    pub(in super::super) fn fixed_array_cells(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(ArrayInfo, IrFixedArrayCells)>, String> {
        match self.kind(node) {
            NodeKind::Array { .. }
            | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => {
                let Some(array) = self.array_of(node).cloned() else {
                    return Ok(None);
                };
                if array.is_net || array.dims.is_empty() {
                    return Ok(None);
                }
                let ir = self.reference_array(array.ir);
                if self.model.arrays[ir].dims != array.dims {
                    return Ok(None);
                }
                Ok(Some((
                    array,
                    IrFixedArrayCells {
                        array: ir,
                        prefix: Vec::new(),
                    },
                )))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (base, indices) = (*base, indices.clone());
                self.fixed_array_cell_selection(path, base, &indices)
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (base, index) = (*base, *index);
                self.fixed_array_cell_selection(path, base, &[index])
            }
            _ => Ok(None),
        }
    }

    fn fixed_array_cell_selection(
        &mut self,
        path: &str,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(ArrayInfo, IrFixedArrayCells)>, String> {
        let Some((array, mut cells)) = self.fixed_array_cells(path, base)? else {
            return Ok(None);
        };
        // Selecting every unpacked dimension names one element, not cells.
        if cells.prefix.len().saturating_add(indices.len()) >= array.dims.len() {
            return Ok(None);
        }
        for index in indices {
            let value = self.lower_expr(path, *index)?;
            if value.is_real() {
                return Err(format!(
                    "fixed unpacked-array index in `{path}` must be an integral expression"
                ));
            }
            cells.prefix.push(value);
        }
        Ok(Some((array, cells)))
    }

    /// Whether a receiver's storage root is a `const ref` formal (13.5.2).
    fn const_ref_receiver(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(
                ExprKind::ArraySelect { base, .. } | ExprKind::BitSelect { base, .. },
            ) => self.const_ref_receiver(*base),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => matches!(
                self.kind(*target),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    const_ref: true,
                    ..
                }
            ),
            _ => false,
        }
    }

    /// Lower `reverse`, `sort` or `rsort` of stored cells into one in-place
    /// cell-wise operation when the receiver names storage whose elements
    /// match `immediate`. Small dense selections keep the straight-line form.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_fixed_array_order_cells(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        method: IrFixedArrayOrderMethod,
        immediate: &TypeDescriptor,
        bounds: (i32, i32),
        with_node: Option<NodeId>,
    ) -> Result<Option<IrStmt>, String> {
        let Some((array, cells)) = self.fixed_array_cells(path, receiver)? else {
            return Ok(None);
        };
        if self.const_ref_receiver(receiver) {
            return Err(format!(
                "fixed-array ordering in `{path}` would modify a const ref formal; its receiver is not writable"
            ));
        }
        let storage = &self.model.arrays[cells.array];
        if array.real || !storage.net_elements.is_empty() {
            return Ok(None);
        }
        let Some(shape) = cells.shape(&self.model) else {
            return Ok(None);
        };
        let element_width = u64::from(storage.elem_width).checked_mul(shape.element_cells);
        if (shape.left, shape.right) != bounds
            || element_width != Self::fixed_descriptor_width_bits(immediate)
        {
            return Ok(None);
        }
        if !storage.sparse() && shape.count <= FIXED_CELL_UNROLL_LIMIT {
            return Ok(None);
        }
        let item_name = self.new_fn_name(path, "order_item");
        let index_name = self.new_fn_name(path, "order_index");
        let key = match with_node {
            Some(with_node) => {
                let iterator = self.db.method_call_iterator(call).ok_or_else(|| {
                    format!("fixed-array sort in `{path}` has no captured iterator binding")
                })?;
                let saved = self.fixed_method_iterators.insert(
                    iterator,
                    FixedMethodIterator {
                        descriptor: immediate.clone(),
                        dimensions: vec![bounds],
                        index_names: vec![index_name.clone()],
                        item_name: item_name.clone(),
                    },
                );
                let mapped = self.lower_expr(path, with_node);
                if let Some(saved) = saved {
                    self.fixed_method_iterators.insert(iterator, saved);
                } else {
                    self.fixed_method_iterators.remove(&iterator);
                }
                let mapped = mapped?;
                if mapped.is_real() || mapped.width == 0 {
                    return Err(format!(
                        "fixed-array sort with expression in `{path}` must produce an integral value"
                    ));
                }
                let (width, signed) = (mapped.width, mapped.signed);
                Some(ir_to_storage(
                    mapped,
                    width,
                    signed,
                    self.db.is_two_state_type(with_node),
                )?)
            }
            None => None,
        };
        if method != IrFixedArrayOrderMethod::Reverse
            && element_width.is_none_or(|width| width > u64::from(LLG_MAX_WIDTH))
        {
            return Err(format!(
                "fixed-array sort in `{path}` compares {} elements wider than the packed value capacity of {} bits",
                element_width.map_or_else(|| "overflowing".to_owned(), |width| width.to_string()),
                LLG_MAX_WIDTH
            ));
        }
        Ok(Some(IrStmt::FixedArrayOrder(Box::new(IrFixedArrayOrder {
            cells,
            method,
            item_name,
            index_name,
            item_signed: immediate.info.signed,
            item_two_state: immediate.two_state,
            key,
        }))))
    }
}
