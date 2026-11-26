//! Whole-array equality of resizable containers and fixed-array views.

use super::*;

impl Codegen<'_> {
    /// `a == b`, `a != b`, `a === b` or `a !== b` where both operands are
    /// whole dynamic arrays, queues or fixed-array views (SV 7.6, 11.4.5):
    /// one runtime comparison of every element in index order.
    pub(in super::super) fn lower_container_equality(
        &mut self,
        path: &str,
        op: Operation,
        operands: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let (case, negate) = match op {
            Operation::Equal => (false, false),
            Operation::NotEqual => (false, true),
            Operation::CaseEqual => (true, false),
            Operation::CaseNotEqual => (true, true),
            _ => return Ok(None),
        };
        let [left, right] = operands else {
            return Ok(None);
        };
        if self.is_fixed_view_slice(*left) || self.is_fixed_view_slice(*right) {
            let packed = [*left, *right].into_iter().any(|operand| {
                let operand = self.p30_unwrap_cast(operand);
                let base = match self.kind(operand) {
                    NodeKind::Expr(
                        ExprKind::ArraySelect { base, .. }
                        | ExprKind::PartSelect { base, .. }
                        | ExprKind::IndexedPartSelect { base, .. },
                    ) => *base,
                    _ => return false,
                };
                self.container_of_select(operand, base)
                    .is_some_and(|view| self.model.containers[view.ir].element.is_packed())
            });
            if packed {
                return Err(crate::sim::codegen::error::located(
                    &self.source_location(*left),
                    format!(
                        "equality of a row or slice of a multidimensional class property in `{path}` is not supported; compare elements or copy the row into an array variable first (SIM-011)"
                    ),
                ));
            }
            return Err(format!(
                "equality of a row or slice of a fixed array of strings, handles or native records in `{path}` is not supported; compare whole arrays or elements"
            ));
        }
        let left_container = self.container_of(self.p30_unwrap_cast(*left));
        let right_container = self.container_of(self.p30_unwrap_cast(*right));
        let (left, right) = match (left_container, right_container) {
            (Some(left), Some(right)) => (left.ir, right.ir),
            (None, None) => return Ok(None),
            // A fixed view of packed words beside a fixed array compares
            // as the two packed values, which is the element-by-element
            // result: any known differing bit gives 0, else any unknown
            // bit gives X (SV 11.2.2, 11.4.5).
            (Some(view), None) | (None, Some(view))
                if self.is_fixed_view(view.ir)
                    && self.model.containers[view.ir].element.is_packed() =>
            {
                return Ok(None)
            }
            _ => {
                return Err(format!(
                    "unpacked array equality in `{path}` needs array variables on both sides"
                ))
            }
        };
        let (left_storage, right_storage) =
            (&self.model.containers[left], &self.model.containers[right]);
        let same_kind = matches!(
            (&left_storage.kind, &right_storage.kind),
            (IrContainerKind::Dynamic, IrContainerKind::Dynamic)
                | (IrContainerKind::Queue { .. }, IrContainerKind::Queue { .. })
        );
        if !same_kind
            || left_storage.element.is_packed() != right_storage.element.is_packed()
            || !left_storage.element.compatible_with(&right_storage.element)
        {
            return Err(format!(
                "unpacked array equality in `{path}` between different container kinds or element types is not supported"
            ));
        }
        Ok(Some(IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Equal {
                left,
                right,
                case,
                negate,
            })),
            1,
            false,
            None,
        )))
    }
}
