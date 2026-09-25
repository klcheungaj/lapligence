//! Stable typed receivers for ordering fixed activation/formal arrays.

use super::*;

impl Codegen<'_> {
    pub(super) fn capture_fixed_ordering_receiver(
        &mut self,
        path: &str,
        receiver: NodeId,
        width: u32,
        method: &str,
        statements: &mut Vec<IrStmt>,
    ) -> Result<(IrExpr, IrLhs), String> {
        let target = self
            .fixed_activation_lhs(path, receiver)?
            .ok_or_else(|| format!("fixed-array {method} in `{path}` has no writable storage"))?;
        let tag = self.new_fn_name(path, "ordering_receiver");
        let mut captures = Vec::new();
        let mut sequence = 0;
        let (target, source) =
            self.freeze_call_lhs(target, &tag, &mut sequence, &mut captures)?;
        // Never relower the original receiver as a fallback: it would embed
        // executable selectors again rather than use the frozen coordinates.
        let source = source.ok_or_else(|| {
            format!("fixed-array {method} in `{path}` has no stable readable storage")
        })?;
        if source.is_real() || source.width != width {
            return Err(format!(
                "array method `{method}` in `{path}` has an unsupported fixed-array representation"
            ));
        }
        // A failed collected-array probe may have prepared a speculative
        // prefix. Only this chosen activation receiver's captures may execute.
        statements.clear();
        statements.extend(captures.into_iter().map(
            |(name, width, signed, two_state, expr)| IrStmt::DeclLocal {
                name,
                width,
                signed,
                two_state,
                init: Some(Box::new(expr)),
            },
        ));
        // Sort rereads current storage through this expression after swaps;
        // reverse takes its own value snapshot. Both share the same selectors.
        Ok((source, target))
    }
}
