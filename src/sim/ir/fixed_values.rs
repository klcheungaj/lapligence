//! Descriptor operands keep fixed values separate from packed-value capacity.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum IrFixedValue {
    Array(IrMemoryView),
    Call {
        array: usize,
        call: Box<IrCall>,
    },
    Conditional {
        selector: Box<IrExpr>,
        left: Box<IrFixedValue>,
        right: Box<IrFixedValue>,
        /// Number of scalar storage cells in one immediate array element.
        element_cells: u64,
    },
    Stream {
        parts: Vec<IrFixedValue>,
        slice: u32,
    },
    /// Explicit bit-stream cast into `array`'s lexical shape (SV 6.24.3):
    /// a reshaped extent or cell width, or a two-state element domain that
    /// must clear X/Z before the value is compared or stored.
    Convert {
        value: Box<IrFixedValue>,
        array: usize,
    },
    /// A packed bit-stream operand of a descriptor stream (a dense array,
    /// integral value, nested packed stream or `with` selection of a dense or
    /// image array) as `cell_width`-bit cells, left cell first. A
    /// `runtime_sized` value learns its width when evaluated; an empty value
    /// contributes no bits.
    Packed {
        value: Box<IrExpr>,
        cell_width: u32,
        runtime_sized: bool,
    },
    /// A runtime-sized bit stream operand (a string, resizable container or
    /// record with such members) as `cell_width`-bit cells, left cell first;
    /// it never forms one packed value, so it is not bounded by the packed
    /// width. An empty stream contributes no bits. With `consume`, it is the
    /// source of an unpack (SV 11.4.14.3): its leftmost `consume` bits are
    /// taken (fewer is a runtime error) before `stream.unpack` reorders them.
    BitStream {
        stream: Box<IrBitStream>,
        cell_width: u32,
        consume: Option<u64>,
    },
    /// A view of dense (below-threshold, non-net) integral array storage,
    /// presented to descriptor transport cell for cell; the source code stays
    /// one copy loop whatever the extent.
    Dense(IrMemoryView),
    /// A one-dimensional descriptor array selected by a runtime or partly
    /// out-of-bounds `with` range (SV 11.4.14.4), streamed in storage order.
    /// Indices outside the bounds read the element's default-uninitialized
    /// value; no cell is materialized.
    Selected {
        array: usize,
        selector: IrStreamSelector,
    },
}

impl IrFixedValue {
    pub(in crate::sim) fn calls(&self, visit: &mut impl FnMut(&IrCall)) {
        match self {
            Self::Array(_) => {}
            Self::Call { call, .. } => {
                for argument in &call.args {
                    if let IrCallArg::FixedValue(value) = argument {
                        value.calls(visit);
                    }
                }
                visit(call);
            }
            Self::Conditional { left, right, .. } => {
                left.calls(visit);
                right.calls(visit);
            }
            Self::Stream { parts, .. } => {
                for part in parts {
                    part.calls(visit);
                }
            }
            Self::Convert { value, .. } => value.calls(visit),
            Self::Packed { .. }
            | Self::Selected { .. }
            | Self::Dense(_)
            | Self::BitStream { .. } => {}
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Array(view) => {
                for selector in &view.selectors {
                    visit(&selector.value);
                }
            }
            Self::Call { call, .. } => {
                for argument in &call.args {
                    argument.expressions(visit);
                }
            }
            Self::Conditional {
                selector,
                left,
                right,
                ..
            } => {
                visit(selector);
                left.expressions(visit);
                right.expressions(visit);
            }
            Self::Stream { parts, .. } => {
                for part in parts {
                    part.expressions(visit);
                }
            }
            Self::Convert { value, .. } => value.expressions(visit),
            Self::Packed { value, .. } => visit(value),
            Self::BitStream { stream, .. } => stream.expressions(visit),
            Self::Dense(view) => {
                for selector in &view.selectors {
                    visit(&selector.value);
                }
            }
            Self::Selected { selector, .. } => match selector {
                IrStreamSelector::Index(index) => visit(index),
                IrStreamSelector::Range { left, right } => {
                    visit(left);
                    visit(right);
                }
                IrStreamSelector::Indexed { base, width, .. } => {
                    visit(base);
                    visit(width);
                }
            },
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Array(view) => {
                for selector in &mut view.selectors {
                    visit(&mut selector.value);
                }
            }
            Self::Call { call, .. } => {
                for argument in &mut call.args {
                    argument.expressions_mut(visit);
                }
            }
            Self::Conditional {
                selector,
                left,
                right,
                ..
            } => {
                visit(selector);
                left.expressions_mut(visit);
                right.expressions_mut(visit);
            }
            Self::Stream { parts, .. } => {
                for part in parts {
                    part.expressions_mut(visit);
                }
            }
            Self::Convert { value, .. } => value.expressions_mut(visit),
            Self::Packed { value, .. } => visit(value),
            Self::BitStream { stream, .. } => stream.expressions_mut(visit),
            Self::Dense(view) => {
                for selector in &mut view.selectors {
                    visit(&mut selector.value);
                }
            }
            Self::Selected { selector, .. } => match selector {
                IrStreamSelector::Index(index) => visit(index),
                IrStreamSelector::Range { left, right } => {
                    visit(left);
                    visit(right);
                }
                IrStreamSelector::Indexed { base, width, .. } => {
                    visit(base);
                    visit(width);
                }
            },
        }
    }
}
