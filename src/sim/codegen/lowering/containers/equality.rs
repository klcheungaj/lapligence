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
        let left_container = self.container_of(self.p30_unwrap_cast(*left));
        let right_container = self.container_of(self.p30_unwrap_cast(*right));
        let (left, right) = match (left_container, right_container) {
            (Some(left), Some(right)) => (left.ir, right.ir),
            (None, None) => return Ok(None),
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
