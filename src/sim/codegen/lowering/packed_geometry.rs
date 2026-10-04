//! Packed select geometry shared by every selection lowering path.
//!
//! A select on a packed value addresses elements of the outermost dimension
//! still visible at its base (IEEE 1800-2009 7.4.5, 11.5.1). One element is
//! `stride` bits wide: the product of the remaining dimensions and the
//! element type, so `w[3:2]` of `logic [3:0][7:0] w` covers 16 bits and a
//! part-select of a packed array of structures covers whole structures.
//! Readers, writers, net aliases, net arrays and dependency extents all map
//! element labels to LSB-relative bits through this one helper.

use super::*;
use crate::core::db::PackedRange;

/// The outermost visible packed dimension of a value and its element width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PackedSelectDim {
    pub(super) range: PackedRange,
    /// Bits per element of `range`.
    pub(super) stride: u32,
}

impl PackedSelectDim {
    /// Geometry of `range` on a `width`-bit value. A value without a captured
    /// packed range (a plain integral atom) is numbered `[width-1:0]`.
    pub(super) fn new(width: u32, range: Option<PackedRange>) -> Result<Self, String> {
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
        if width == 0 || extent == 0 || !width.is_multiple_of(extent) {
            return Err("packed dimension disagrees with its element width".to_owned());
        }
        Ok(Self {
            range,
            stride: width / extent,
        })
    }

    pub(super) fn ascending(self) -> bool {
        self.range.left < self.range.right
    }

    /// Element offset of `label` from the least significant element. It is
    /// negative or past the last element for an out-of-range label.
    pub(super) fn element_offset(self, label: i128) -> Option<i128> {
        if self.ascending() {
            self.range.right.checked_sub(label)
        } else {
            label.checked_sub(self.range.right)
        }
    }

    /// Bit offset of the least significant bit of `count` elements whose
    /// least significant element is `back` elements below `label`.
    fn lsb(self, label: i128, back: u32) -> Option<i128> {
        self.element_offset(label)?
            .checked_sub(i128::from(back))?
            .checked_mul(i128::from(self.stride))
    }

    fn width(self, count: u128) -> Result<u32, String> {
        count
            .checked_mul(u128::from(self.stride))
            .and_then(|width| u32::try_from(width).ok())
            .filter(|width| *width != 0 && *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| "packed selection width exceeds the supported limit".to_owned())
    }

    /// Elements below an indexed part-select's base label that the select
    /// also covers: `+:` grows toward the MSB of a descending range and `-:`
    /// toward the LSB, and an ascending range reverses both.
    pub(super) fn indexed_back(self, count: u32, negative: bool) -> u32 {
        if negative ^ self.ascending() {
            count - 1
        } else {
            0
        }
    }

    /// LSB-relative `(lsb, width)` bits of element `label`.
    pub(super) fn element(self, label: i128) -> Result<(i128, u32), String> {
        let lsb = self
            .lsb(label, 0)
            .ok_or_else(|| "packed select offset overflows".to_owned())?;
        Ok((lsb, self.stride))
    }

    /// LSB-relative `(lsb, width)` bits of the constant part-select
    /// `[left:right]`. Its bounds follow the declared direction, so `right`
    /// is always the least significant element.
    pub(super) fn part(self, left: i128, right: i128) -> Result<(i128, u32), String> {
        if left != right && (left < right) != self.ascending() {
            return Err("reversed packed part-select".to_owned());
        }
        let lsb = self
            .lsb(right, 0)
            .ok_or_else(|| "packed select offset overflows".to_owned())?;
        Ok((lsb, self.width(left.abs_diff(right) + 1)?))
    }

    /// LSB-relative `(lsb, width)` bits of the constant indexed part-select
    /// `[base +: count]` or `[base -: count]`.
    pub(super) fn indexed(
        self,
        base: i128,
        count: u32,
        negative: bool,
    ) -> Result<(i128, u32), String> {
        let lsb = self
            .lsb(base, self.indexed_back(count, negative))
            .ok_or_else(|| "packed select offset overflows".to_owned())?;
        Ok((lsb, self.width(u128::from(count))?))
    }

    /// Bit width of an indexed part-select of `count` elements.
    pub(super) fn indexed_width(self, count: u32) -> Result<u32, String> {
        self.width(u128::from(count))
    }

    /// Runtime LSB of the element `index`, less `back` elements.
    pub(super) fn lsb_expr(self, index: IrExpr, back: u32) -> Result<IrExpr, String> {
        packed_lsb(index, self.range, self.stride, back)
    }
}

/// Widen before coordinate arithmetic, including multiplication. An unsigned
/// high-bit index must not become negative or wrap into a valid lane. Selector
/// literals are self-determined; an unbased '1 denotes one, not a widened fill.
pub(super) fn packed_lsb(
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
            IrExprKind::Concat { parts: vec![index] },
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
        .ok_or_else(|| {
            "packed selection index arithmetic exceeds the supported limit".to_owned()
        })?;
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
