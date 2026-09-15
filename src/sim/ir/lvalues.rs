//! Lvalues.

use super::*;

/// An element-level select applied after the array indices of an array read
/// or element write (`mem[i]`, `mem[i][3:0]`, `mem[i][j]`).
#[derive(Clone, Debug, PartialEq)]
pub enum IrElemSel {
    /// Whole element.
    Whole,
    /// Part-select `[left:right]` of the element.
    Part(i64, i64),
    /// Bit-select of the element by a runtime index expression.
    Bit(Box<IrExpr>),
    /// Indexed part-select with a translated runtime base and constant width.
    Indexed {
        base: Box<IrExpr>,
        width: u32,
        negative: bool,
    },
}

/// Direction of a packed streaming concatenation (LRM 1800-2009 §11.4.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrStreamDirection {
    /// `{>>{...}}`: preserve the left-to-right stream order.
    LeftToRight,
    /// `{<< slice {...}}`: reverse the order of `slice`-bit blocks.
    RightToLeft,
}

/// One scalar integral member of an `inside` set.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInsideItem {
    /// A wildcard-matched value item.
    Value(IrExpr),
    /// An inclusive `[low:high]` range.
    Range { low: IrExpr, high: IrExpr },
    /// An inclusive range with one unbounded endpoint.
    OpenRange {
        low: Option<IrExpr>,
        high: Option<IrExpr>,
    },
    /// All values currently stored in a dynamic, queue, or associative
    /// container. The container storage ABI remains unchanged.
    Container { container: usize },
}

/// Assignment target, mirroring the pre-IR LHS analysis outcomes.
#[derive(Clone, Debug, PartialEq)]
pub enum IrLhs {
    /// Whole signal (packed global, real companion, or a collapsed-net member
    /// reached through `model.signals[..].net_driver`).
    Whole(usize),
    /// A complete C address/lvalue expression (a `sv4_t*` parameter such as
    /// `o0`, or `&_l3` for a local); emitted verbatim, no `&` prepended.
    WholeRef {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        /// Real storage rounds through `float` when this is a shortreal.
        shortreal: bool,
    },
    /// A subroutine `ref` formal.  The address names an `llg_ref_t` descriptor
    /// and writes are committed through its canonical target immediately.
    Ref {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        /// Whether this descriptor is read-only because it names a `const
        /// ref` formal in the enclosing activation.
        const_ref: bool,
        /// Optional runtime bit index, translated to the descriptor's packed
        /// value coordinates. A selected reference has width one and is unsigned.
        bit: Option<Box<IrExpr>>,
    },
    /// Bit-select `[idx]` of a signal.
    Bit(usize, IrExpr, bool),
    /// Part-select `[left:right]` of a signal (constant bounds).
    Part(usize, i64, i64, bool),
    /// Indexed part-select `[base +: width]` / `[base -: width]`
    /// The explicit `u32` is the constant selected width; `neg` selects the
    /// descending form.  Keeping the selected width separate from the width
    /// expression's own type lets capacity analysis account for `[base +: N]`
    /// even when `N` is represented by a narrow integer expression.
    IdxPart(usize, IrExpr, IrExpr, u32, bool, bool),
    /// One unpacked-array element with an optional element-level select;
    /// emitted as a guarded statement (out-of-range/unknown indices no-op).
    ArrayElem {
        arr: usize,
        indices: Vec<IrExpr>,
        elem_sel: IrElemSel,
    },
    /// Streaming concatenation assignment target. Each part's explicit width
    /// preserves the static unpack shape independently of the target storage.
    Stream {
        parts: Vec<(IrLhs, u32)>,
        width: u32,
        slice: u32,
        direction: IrStreamDirection,
    },
}

/// One component of a streaming assignment target. Packed lvalues have a
/// statically known width; resizable packed-element containers retain their
/// selector so the emitter can derive their width from the captured source
/// expression at runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStreamTarget {
    Packed {
        lhs: IrLhs,
        width: u32,
    },
    Container {
        container: usize,
        selector: Option<IrStreamSelector>,
    },
}

impl IrLhs {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Ref {
                bit: Some(index), ..
            } => visit(index),
            Self::Bit(_, index, _) => visit(index),
            Self::IdxPart(_, base, width, ..) => {
                visit(base);
                visit(width);
            }
            Self::ArrayElem {
                indices, elem_sel, ..
            } => {
                for index in indices {
                    visit(index);
                }
                match elem_sel {
                    IrElemSel::Bit(index) => visit(index),
                    IrElemSel::Indexed { base, .. } => visit(base),
                    IrElemSel::Whole | IrElemSel::Part(..) => {}
                }
            }
            Self::Stream { parts, .. } => {
                for (part, _) in parts {
                    part.expressions(visit);
                }
            }
            Self::Whole(..) | Self::WholeRef { .. } | Self::Ref { .. } | Self::Part(..) => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Ref {
                bit: Some(index), ..
            } => visit(index),
            Self::Bit(_, index, _) => visit(index),
            Self::IdxPart(_, base, width, ..) => {
                visit(base);
                visit(width);
            }
            Self::ArrayElem {
                indices, elem_sel, ..
            } => {
                for index in indices {
                    visit(index);
                }
                match elem_sel {
                    IrElemSel::Bit(index) => visit(index),
                    IrElemSel::Indexed { base, .. } => visit(base),
                    IrElemSel::Whole | IrElemSel::Part(..) => {}
                }
            }
            Self::Stream { parts, .. } => {
                for (part, _) in parts {
                    part.expressions_mut(visit);
                }
            }
            Self::Whole(..) | Self::WholeRef { .. } | Self::Ref { .. } | Self::Part(..) => {}
        }
    }
}
