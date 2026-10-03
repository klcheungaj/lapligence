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
        }
    }
}
