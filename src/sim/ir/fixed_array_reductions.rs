//! Fixed-array method reductions retain element boundaries and lexical evaluation.

use super::*;

/// Receiver of a fixed unpacked-array reduction. Neither variant creates a
/// resizable SystemVerilog container.
#[derive(Clone, Debug, PartialEq)]
pub enum IrFixedArrayReductionSource {
    /// Whole one-dimensional model storage. Read cells in declaration order;
    /// the total array payload need not fit in one packed runtime value.
    Array(usize),
    /// A declaration-order payload (activation, slice, row, record member or
    /// function result). Evaluate and capture this expression exactly once.
    Value(Box<IrExpr>),
}

/// A nonempty fixed-array fold. `value` is evaluated once per immediate
/// unpacked element in a scope binding `item_name` and `index_name`. Nested
/// reductions create nested scopes; no callback loses the enclosing activation.
///
/// The first mapped value seeds the accumulator unchanged, including X/Z.
/// Later values use the selected binary operation. Both the mapped value and
/// accumulator keep the method's self-determined width and signedness, never
/// the enclosing assignment's width (IEEE 1800-2009 7.12.3).
#[derive(Clone, Debug, PartialEq)]
pub struct IrFixedArrayReduction {
    pub source: IrFixedArrayReductionSource,
    pub operation: IrContainerReduction,
    /// Bounds of the receiver's outermost unpacked dimension.
    pub left: i32,
    pub right: i32,
    /// Flattened width and signedness of one immediate unpacked element.
    pub element_width: u32,
    pub element_signed: bool,
    pub element_two_state: bool,
    /// Internal lexical binding names; these are not emitted C fragments.
    pub item_name: String,
    pub index_name: String,
    /// Integral map expression, or the item read when `with` is omitted.
    pub value: IrExpr,
}

impl IrFixedArrayReduction {
    pub(in crate::sim) fn element_count(&self) -> Option<u64> {
        i64::from(self.left)
            .abs_diff(i64::from(self.right))
            .checked_add(1)
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let IrFixedArrayReductionSource::Value(value) = &self.source {
            visit(value);
        }
        visit(&self.value);
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let IrFixedArrayReductionSource::Value(value) = &mut self.source {
            visit(value);
        }
        visit(&mut self.value);
    }
}
