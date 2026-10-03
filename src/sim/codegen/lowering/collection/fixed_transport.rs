//! Fixed values lower to descriptor operations rather than leaf expansion.
use super::*;
use crate::sim::ir::IrFixedValue;

impl Codegen<'_> {
    pub(in super::super) fn lower_fixed_value(&mut self, path: &str, node: NodeId) -> Result<IrFixedValue, String> {
        let node = self.p30_unwrap_cast(node);
        if let NodeKind::FuncCall { name, callee, .. } = self.kind(node) {
            let (name, callee) = (name.clone(), *callee);
            let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
            if self.nonflatten_function(function) {
                let result = self.fixed_activation_array(function)?;
                let expression = self.lower_func_call_expr(path, node, &name, callee)?;
                let IrExprKind::CallFn(expression) = expression.kind else { return Err("fixed call requires typed operands".into()); };
                let mut args = expression.args;
                let outputs = self.model.funcs[expression.f].formals.iter().filter(|formal| formal.is_address()).count();
                args.insert(outputs - 1, IrCallArg::FixedArray(result.ir));
                return Ok(IrFixedValue::Call { array: result.ir, call: Box::new(IrCall::new(expression.f, args, expression.depth, Vec::new(), Vec::new())) });
            }
        }
        if let NodeKind::Expr(ExprKind::Operation { op: Operation::Conditional, operands, .. }) = self.kind(node) {
            let operands = operands.clone();
            let selector = self.lower_boolean_expr(path, operands[0])?;
            let left = self.lower_fixed_value(path, operands[1])?;
            let right = self.lower_fixed_value(path, operands[2])?;
            let descriptor = self.query_descriptor(node).ok_or("fixed conditional has no shape")?;
            let element_cells = if let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape {
                dimensions.iter().skip(1).try_fold(1u64, |total, (left, right)| total.checked_mul(u64::from(left.abs_diff(*right)) + 1)).ok_or("fixed conditional element count overflows")?
            } else { 1 };
            return Ok(IrFixedValue::Conditional { selector: Box::new(selector), left: Box::new(left), right: Box::new(right), element_cells });
        }
        if let NodeKind::Expr(ExprKind::Streaming { direction, slice_size, streams }) = self.kind(node) {
            let slice = if *direction == DbStreamingDirection::LeftToRight { 0 } else { u32::try_from((*slice_size).max(1)).map_err(|_| "fixed stream slice overflows")? };
            let streams = streams.clone();
            let mut parts = Vec::new();
            for stream in streams {
                if stream.with_expr.is_some() { return Err("fixed descriptor stream with selection requires a fixed array slice".into()); }
                parts.push(self.lower_fixed_value(path, stream.value)?);
            }
            return Ok(IrFixedValue::Stream { parts, slice });
        }
        let view = self.fixed_memory_view(path, node)?;
        if !self.model.arrays[view.array].sparse() { return Err("descriptor value requires descriptor storage".into()); }
        Ok(IrFixedValue::Array(view))
    }
}
