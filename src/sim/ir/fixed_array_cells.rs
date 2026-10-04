//! Cell-wise views and in-place ordering of stored fixed unpacked arrays.
//!
//! These operations read and write the selected cells of one model or
//! activation array directly, in declaration order. They never form one packed
//! value for the receiver, so their generated code and runtime work do not
//! depend on the payload capacity, and descriptor-backed arrays keep their
//! sparse storage.

use super::*;

/// The cells of one stored fixed array below a selected prefix of its leading
/// unpacked dimensions. Selectors are evaluated once, in order, before any
/// cell is touched; an unknown or out-of-range selector selects no storage.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFixedArrayCells {
    pub array: usize,
    /// Declared-index selectors for `dims[..prefix.len()]`.
    pub prefix: Vec<IrExpr>,
}

impl IrFixedArrayCells {
    /// Declared bounds of the first unselected dimension, the cells of one
    /// element of that dimension, and its element count.
    pub(in crate::sim) fn shape(&self, model: &IrModel) -> Option<FixedArrayCellShape> {
        let array = model.arrays.get(self.array)?;
        let (&(left, right), trailing) = array.dims.get(self.prefix.len()..)?.split_first()?;
        let count = i64::from(left).abs_diff(i64::from(right)).checked_add(1)?;
        let element_cells = trailing.iter().try_fold(1u64, |cells, (left, right)| {
            cells.checked_mul(
                i64::from(*left)
                    .abs_diff(i64::from(*right))
                    .checked_add(1)?,
            )
        })?;
        Some(FixedArrayCellShape {
            left,
            right,
            count,
            element_cells,
            cells: count.checked_mul(element_cells)?,
        })
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        self.prefix.iter().for_each(visit);
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        self.prefix.iter_mut().for_each(visit);
    }
}

/// Static geometry of an [`IrFixedArrayCells`] view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim) struct FixedArrayCellShape {
    pub left: i32,
    pub right: i32,
    /// Elements of the first unselected dimension.
    pub count: u64,
    /// Cells per element; greater than one when elements are rows.
    pub element_cells: u64,
    /// All selected cells.
    pub cells: u64,
}

/// In-place fixed-array ordering method (IEEE 1800-2009 7.12.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrFixedArrayOrderMethod {
    Reverse,
    Sort,
    RSort,
}

/// Reorder the elements of the first unselected dimension of `cells`; rows
/// move whole. Sorting evaluates `key` (or reads the element when `key` is
/// absent) once per element in declaration order, binding `item_name` to the
/// element and `index_name` to its declared index, and then moves elements to
/// a stable order of those keys. Elements with unknown keys keep their
/// positions; their relative order is unspecified by the standard. Every moved
/// cell is published through the ordinary blocking-store path.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFixedArrayOrder {
    pub cells: IrFixedArrayCells,
    pub method: IrFixedArrayOrderMethod,
    /// Internal lexical binding names; these are not emitted C fragments.
    pub item_name: String,
    pub index_name: String,
    /// Element view type; its width is the selected row width.
    pub item_signed: bool,
    pub item_two_state: bool,
    pub key: Option<IrExpr>,
}

impl IrFixedArrayOrder {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        self.cells.expressions(visit);
        if let Some(key) = &self.key {
            visit(key);
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        self.cells.expressions_mut(visit);
        if let Some(key) = &mut self.key {
            visit(key);
        }
    }
}
