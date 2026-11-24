//! Selects of two-state vectors (S07-D7).
//!
//! A select that addresses a position outside its vector, or whose index is
//! unknown, has no bit to read. The runtime select functions supply X for such
//! a bit (IEEE 1800-2009 11.5.1), which a two-state select cannot hold: its
//! value is 0, the default of its type (7.4.6). Lowering marks these selects
//! with a two-state conversion, so the runtime functions stay the ones a
//! four-state select uses. A select whose positions all lie inside the vector
//! needs no conversion.

use super::*;
use crate::sim::ir::IrPackedSelect;

/// Inclusive bounds of a known integer.
type Interval = (i128, i128);

impl Codegen<'_> {
    /// Give the lowered select expression `node` the state domain of its type.
    /// Any other expression is returned unchanged.
    pub(in crate::sim::codegen) fn settle_two_state_select(
        &self,
        node: NodeId,
        value: IrExpr,
    ) -> IrExpr {
        if self.select_may_miss(&value)
            && matches!(
                self.kind(node),
                NodeKind::Expr(
                    ExprKind::BitSelect { .. }
                        | ExprKind::PartSelect { .. }
                        | ExprKind::IndexedPartSelect { .. }
                        | ExprKind::ArraySelect { .. }
                        | ExprKind::MemberSelect { .. }
                        | ExprKind::HierPath { .. }
                )
            )
            && self.db.is_two_state_type(node)
        {
            IrExpr::to_two_state(value)
        } else {
            value
        }
    }

    /// Whether `value` selects positions that are not all known to lie inside
    /// the selected value.
    fn select_may_miss(&self, value: &IrExpr) -> bool {
        match &value.kind {
            IrExprKind::BitSel { base, idx } => {
                !self.span_inside(idx, 0, 0, base.width) || self.select_may_miss(base)
            }
            IrExprKind::PartSel { base, left, right } => {
                !part_inside(*left, *right, base.width) || self.select_may_miss(base)
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                neg,
                ..
            } => {
                let (below, above) = indexed_span(value.width, *neg);
                !self.span_inside(base_idx, below, above, base.width) || self.select_may_miss(base)
            }
            IrExprKind::ArrayRead { arr, elem_sel, .. } => {
                let Some(array) = self.model.arrays.get(self.reference_array(*arr)) else {
                    return true;
                };
                match elem_sel {
                    IrElemSel::Whole => false,
                    IrElemSel::Bit(index) => !self.span_inside(index, 0, 0, array.elem_width),
                    IrElemSel::Part(left, right) => !part_inside(*left, *right, array.elem_width),
                    IrElemSel::Indexed {
                        base,
                        width,
                        negative,
                    } => {
                        let (below, above) = indexed_span(*width, *negative);
                        !self.span_inside(base, below, above, array.elem_width)
                    }
                    IrElemSel::PackedChain(steps) => self.chain_may_miss(steps, array.elem_width),
                }
            }
            IrExprKind::TaggedSelect { base, steps, .. } => {
                let mut width = base.width;
                steps.iter().any(|step| {
                    let outside = std::mem::replace(&mut width, step.selection.width);
                    self.step_may_miss(&step.selection, outside)
                })
            }
            IrExprKind::Resize { a } | IrExprKind::Convert { a } => self.select_may_miss(a),
            _ => false,
        }
    }

    fn chain_may_miss(&self, steps: &[IrPackedSelect], mut width: u32) -> bool {
        steps.iter().any(|step| {
            let outside = std::mem::replace(&mut width, step.width);
            self.step_may_miss(step, outside)
        })
    }

    fn step_may_miss(&self, step: &IrPackedSelect, width: u32) -> bool {
        let (below, above) = indexed_span(step.width, false);
        !self.span_inside(&step.base, below, above, width)
    }

    /// Whether the positions from `index - below` to `index + above` are known
    /// and inside a value of `width` bits.
    fn span_inside(&self, index: &IrExpr, below: i128, above: i128, width: u32) -> bool {
        self.known_interval(index)
            .is_some_and(|(low, high)| low - below >= 0 && high + above < i128::from(width))
    }

    /// Bounds of an index that is never unknown, when its form shows them.
    fn known_interval(&self, index: &IrExpr) -> Option<Interval> {
        // Every bound below fits 64 bits, so sums and products of two of them
        // fit an i128.
        if index.width == 0 || index.width > 64 {
            return None;
        }
        let range = if index.signed {
            let limit = 1i128 << (index.width - 1);
            (-limit, limit - 1)
        } else {
            (0, (1i128 << index.width) - 1)
        };
        let interval = match &index.kind {
            IrExprKind::Const(_) => {
                let value = Self::ir_constant_i128(index)?;
                (value, value)
            }
            IrExprKind::SigRead(signal) => {
                if !self.model.signals.get(*signal)?.ty.two_state() {
                    return None;
                }
                range
            }
            IrExprKind::ToTwoState { .. } => range,
            IrExprKind::Resize { a } | IrExprKind::Convert { a } => self.known_interval(a)?,
            IrExprKind::Bin { op, a, b } => {
                let (a, b) = (self.known_interval(a)?, self.known_interval(b)?);
                match op {
                    IrBinOp::Add => (a.0 + b.0, a.1 + b.1),
                    IrBinOp::Sub => (a.0 - b.1, a.1 - b.0),
                    IrBinOp::Mul if a.0 >= 0 && b.0 >= 0 => (a.0 * b.0, a.1 * b.1),
                    _ => return None,
                }
            }
            _ => return None,
        };
        // A value outside the expression's own range wraps.
        (interval.0 >= range.0 && interval.1 <= range.1).then_some(interval)
    }
}

/// Positions below and above the base of an indexed part-select.
fn indexed_span(width: u32, negative: bool) -> (i128, i128) {
    let span = i128::from(width) - 1;
    if negative {
        (span, 0)
    } else {
        (0, span)
    }
}

fn part_inside(left: i64, right: i64, width: u32) -> bool {
    left.min(right) >= 0 && left.max(right) < i64::from(width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::IrSignal;

    fn packed(width: u32, signed: bool, two_state: bool) -> IrType {
        IrType::Packed {
            width,
            signed,
            two_state,
        }
    }

    fn read(signal: usize, width: u32, signed: bool) -> IrExpr {
        IrExpr::new(IrExprKind::SigRead(signal), width, signed, None)
    }

    fn bit(idx: IrExpr) -> IrExpr {
        IrExpr::new(
            IrExprKind::BitSel {
                base: Box::new(read(0, 8, false)),
                idx: Box::new(idx),
            },
            1,
            false,
            None,
        )
    }

    fn indexed(base_idx: IrExpr, width: u32, neg: bool) -> IrExpr {
        IrExpr::new(
            IrExprKind::IdxPartSel {
                base: Box::new(read(0, 8, false)),
                base_idx: Box::new(base_idx),
                width_expr: Box::new(lhs_integer_expr(i128::from(width))),
                neg,
            },
            width,
            false,
            None,
        )
    }

    fn part(left: i64, right: i64) -> IrExpr {
        IrExpr::new(
            IrExprKind::PartSel {
                base: Box::new(read(0, 8, false)),
                left,
                right,
            },
            left.abs_diff(right) as u32 + 1,
            false,
            None,
        )
    }

    #[test]
    fn component_only_selects_inside_the_value_are_certain() {
        let db = Db::from_test_nodes("selects", Vec::new(), Vec::new(), HashMap::new()).unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let mut cg = Codegen::new(&semantic);
        for (name, ty) in [
            ("v", packed(8, false, true)),
            ("bits3", packed(3, false, true)),
            ("bits4", packed(4, false, true)),
            ("logic3", packed(3, false, false)),
            ("int32", packed(32, true, true)),
        ] {
            cg.model
                .signals
                .push(IrSignal::new(format!("G_{name}"), None, ty, None).unwrap());
        }
        let number = lhs_integer_expr;
        let offset = |index: IrExpr, low: i128| {
            let index = IrExpr::convert_to(index, 34, true);
            bin_expr(
                IrBinOp::Sub,
                index,
                IrExpr::convert_to(number(low), 34, true),
            )
        };
        let inside = [
            bit(number(0)),
            bit(number(7)),
            bit(read(1, 3, false)),
            bit(offset(number(15), 8)),
            part(7, 0),
            part(3, 5),
            indexed(number(4), 4, false),
            indexed(number(3), 4, true),
            bit(IrExpr::to_two_state(read(3, 3, false))),
        ];
        let outside = [
            bit(number(8)),
            bit(number(-1)),
            bit(read(2, 4, false)),
            bit(read(3, 3, false)),
            bit(read(4, 32, true)),
            bit(offset(number(16), 8)),
            bit(offset(read(1, 3, false), 1)),
            part(8, 5),
            part(3, -1),
            indexed(number(5), 4, false),
            indexed(number(2), 4, true),
            indexed(read(1, 3, false), 2, false),
        ];
        for select in &inside {
            assert!(!cg.select_may_miss(select), "{select:?}");
        }
        for select in &outside {
            assert!(cg.select_may_miss(select), "{select:?}");
        }
    }
}
