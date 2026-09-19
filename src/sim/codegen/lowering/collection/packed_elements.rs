//! Packed selections of fixed-array elements, preserving each intermediate bound.

use super::*;
use crate::core::db::PackedRange;
use crate::sim::ir::IrPackedSelect;

pub(super) enum Select {
    Elements(Vec<NodeId>),
    Part(NodeId, NodeId),
    Indexed(NodeId, NodeId, bool),
}

impl<'a> Codegen<'a> {
    /// Retain the typed slice chain rather than flattening offsets across
    /// intermediate bounds. The same plan feeds reads, mutations and masked
    /// NBA writes. A fully indexed unpacked root is required.
    pub(in super::super) fn packed_element_lhs_ir(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let mut current = node;
        let mut outer = Vec::new();
        let (array_node, array_indices, packed_indices) = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    if let Some(array) = self.array_of(*base) {
                        let rank = array.dims.len();
                        if indices.len() < rank || array.real {
                            return Ok(None);
                        }
                        break (*base, indices[..rank].to_vec(), indices[rank..].to_vec());
                    }
                    outer.push((*base, Select::Elements(indices.clone())));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    if let Some(array) = self.array_of(*base) {
                        if array.dims.len() != 1 || array.real {
                            return Ok(None);
                        }
                        break (*base, vec![*index], Vec::new());
                    }
                    outer.push((*base, Select::Elements(vec![*index])));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    outer.push((*base, Select::Part(*left, *right)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => {
                    outer.push((*base, Select::Indexed(*base_expr, *width_expr, *neg)));
                    current = *base;
                }
                _ => return Ok(None),
            }
        };
        if outer.is_empty() && packed_indices.is_empty() {
            return Ok(None);
        }
        let array = self
            .array_of(array_node)
            .cloned()
            .ok_or_else(|| format!("packed selection lost its array root in `{path}`"))?;
        let indices = array_indices
            .iter()
            .map(|index| self.lower_expr(path, *index))
            .collect::<Result<Vec<_>, _>>()?;
        let mut steps = Vec::new();
        let mut parent_width = array.elem_width;
        self.packed_selection_steps(
            path,
            array_node,
            Select::Elements(packed_indices),
            &mut parent_width,
            &mut steps,
        )?;
        for (base, select) in outer.into_iter().rev() {
            self.packed_selection_steps(path, base, select, &mut parent_width, &mut steps)?;
        }
        Ok(Some(IrLhs::ArrayElem {
            arr: self.reference_array(array.ir),
            indices,
            elem_sel: IrElemSel::PackedChain(steps),
        }))
    }

    pub(super) fn packed_selection_steps(
        &mut self,
        path: &str,
        base: NodeId,
        select: Select,
        parent_width: &mut u32,
        steps: &mut Vec<IrPackedSelect>,
    ) -> Result<(), String> {
        let dimensions = self.db.packed_dimensions(base).unwrap_or(&[]).to_vec();
        match select {
            Select::Elements(indices) => {
                for (dimension, index) in indices.into_iter().enumerate() {
                    let (range, stride) =
                        packed_dimension(*parent_width, dimensions.get(dimension).copied())?;
                    let index = self.lower_expr(path, index)?;
                    steps.push(IrPackedSelect {
                        base: packed_lsb(index, range, stride, 0)?,
                        width: stride,
                    });
                    *parent_width = stride;
                }
            }
            Select::Part(left, right) => {
                let (range, stride) = packed_dimension(*parent_width, dimensions.first().copied())?;
                let left = self.eval_bound_i128(left)?;
                let right = self.eval_bound_i128(right)?;
                if left != right && (left < right) != (range.left < range.right) {
                    return Err(format!("reversed packed part-select in `{path}`"));
                }
                let count = left
                    .abs_diff(right)
                    .checked_add(1)
                    .ok_or_else(|| format!("packed selection extent overflows in `{path}`"))?;
                let width = packed_selection_width(count, stride)?;
                steps.push(IrPackedSelect {
                    base: packed_lsb(lhs_integer_expr(right), range, stride, 0)?,
                    width,
                });
                *parent_width = width;
            }
            Select::Indexed(base, width, negative) => {
                let (range, stride) = packed_dimension(*parent_width, dimensions.first().copied())?;
                let count = self.indexed_part_select_width(width, path)?;
                let width = packed_selection_width(u128::from(count), stride)?;
                // For ascending declarations +: starts at the MSB; for
                // descending declarations -: starts at the MSB. Translate to
                // the LSB before multiplying by the remaining element stride.
                let back = if negative ^ (range.left < range.right) {
                    count - 1
                } else {
                    0
                };
                let base = self.lower_expr(path, base)?;
                steps.push(IrPackedSelect {
                    base: packed_lsb(base, range, stride, back)?,
                    width,
                });
                *parent_width = width;
            }
        }
        Ok(())
    }
}

fn packed_dimension(width: u32, range: Option<PackedRange>) -> Result<(PackedRange, u32), String> {
    let range = match range {
        Some(range) => range,
        None if width > 1 => PackedRange {
            left: i128::from(width - 1),
            right: 0,
        },
        None => return Err("packed selection has no remaining dimension".to_owned()),
    };
    let extent = range
        .left
        .abs_diff(range.right)
        .checked_add(1)
        .and_then(|extent| u32::try_from(extent).ok())
        .ok_or_else(|| "packed dimension extent overflows".to_owned())?;
    if width == 0 || extent == 0 || width % extent != 0 {
        return Err("packed dimension disagrees with its element width".to_owned());
    }
    Ok((range, width / extent))
}

fn packed_selection_width(count: u128, stride: u32) -> Result<u32, String> {
    count
        .checked_mul(u128::from(stride))
        .and_then(|width| u32::try_from(width).ok())
        .filter(|width| *width != 0 && *width <= LLG_MAX_WIDTH)
        .ok_or_else(|| "packed selection width exceeds the supported limit".to_owned())
}

/// Widen before coordinate arithmetic, including multiplication. An unsigned
/// high-bit index must not become negative or wrap into a valid lane. Selector
/// literals are self-determined; an unbased '1 denotes one, not a widened fill.
fn packed_lsb(
    mut index: IrExpr,
    range: PackedRange,
    stride: u32,
    back: u32,
) -> Result<IrExpr, String> {
    if index.is_real() || stride == 0 {
        return Err("packed selection requires an integral index and nonzero stride".to_owned());
    }
    if index.fill.is_some()
        || matches!(&index.kind, IrExprKind::Fill(_))
        || matches!(&index.kind, IrExprKind::Const(value) if value.fill.is_some())
    {
        // A one-item concatenation establishes a self-determined unsigned
        // value in both the constant folder and the owned C emitter. Merely
        // clearing IrExpr::fill leaves a constant's own fill marker active.
        let width = index.width;
        index = IrExpr::new(
            IrExprKind::Concat {
                parts: vec![index],
            },
            width,
            false,
            None,
        );
    }
    let right = lhs_integer_expr(range.right);
    let back = lhs_integer_expr(i128::from(back));
    let multiply_bits = u32::BITS - (stride - 1).leading_zeros();
    let width = index
        .width
        .max(right.width)
        .max(back.width)
        .checked_add(2)
        .and_then(|width| width.checked_add(multiply_bits))
        .filter(|width| *width <= LLG_MAX_WIDTH)
        .ok_or_else(|| "packed selection index arithmetic exceeds the supported limit".to_owned())?;
    let index = IrExpr::convert_to(index, width, true);
    let right = IrExpr::convert_to(right, width, true);
    let offset = if range.left < range.right {
        bin_expr(IrBinOp::Sub, right, index)
    } else {
        bin_expr(IrBinOp::Sub, index, right)
    };
    let offset = bin_expr(IrBinOp::Sub, offset, IrExpr::convert_to(back, width, true));
    Ok(bin_expr(
        IrBinOp::Mul,
        offset,
        IrExpr::convert_to(lhs_integer_expr(i128::from(stride)), width, true),
    ))
}

#[cfg(test)]
mod tests;
