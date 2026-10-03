//! Bounded loops for structurally repeated concatenation operands.
use super::*;

/// Minimum number of identical append operations emitted as a loop.
pub(super) const REPEAT_VALUE_MIN_COUNT: usize = 4;

impl Frame<'_, '_> {
    pub(super) fn concat(&mut self, parts: &[IrExpr]) -> Result<Value, String> {
        let (first, mut rest) = parts
            .split_first()
            .ok_or_else(|| "empty concatenation".to_owned())?;
        let mut result = self.expression(first)?;
        while let Some(part) = rest.first() {
            let count = rest.iter().take_while(|next| *next == part).count();
            if count >= REPEAT_VALUE_MIN_COUNT && repeatable_operand(part) {
                let width = u32::try_from(count)
                    .ok()
                    .and_then(|count| part.width.checked_mul(count))
                    .and_then(|width| result.width.checked_add(width))
                    .ok_or_else(|| "concatenation width overflow".to_owned())?;
                let (ordinal, declaration) = self.loop_variable("uint32_t", "value_repeat");
                self.line(format!(
                    "for ({declaration} = 0; {ordinal} < {count}U; ++{ordinal}) {{"
                ));
                // Keep evaluation and destruction inside the loop: reads and
                // mutations may produce a different value on each iteration.
                result = self.concat_append(result, part)?;
                self.line("}");
                result.width = width;
                rest = &rest[count..];
            } else {
                for part in &rest[..count] {
                    result = self.concat_append(result, part)?;
                }
                rest = &rest[count..];
            }
        }
        // Even a one-element concatenation is unsigned and self-determined.
        self.line(format!("{}.is_signed = 0;", result.code));
        result.signed = false;
        result.fill = None;
        Ok(result)
    }

    fn concat_append(&mut self, result: Value, part: &IrExpr) -> Result<Value, String> {
        let value = self.operand(part)?;
        let width = result
            .width
            .checked_add(value.width)
            .ok_or_else(|| "concatenation width overflow".to_owned())?;
        let code = format!("sv4_concat({}, {})", result.code, value.code);
        let result = self.replace(result, code, width, false);
        self.discard(value);
        Ok(result)
    }
}

fn repeatable_operand(expr: &IrExpr) -> bool {
    if expr.width == 0 {
        return false;
    }
    // Calls and unrepresented kinds stay expanded: each occurrence can own a
    // distinct resume site or lexical storage. These numeric operations return
    // only their result slot; all other expression slots are released locally.
    match &expr.kind {
        IrExprKind::Const(_)
        | IrExprKind::Fill(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_) => true,
        IrExprKind::Un { a, .. }
        | IrExprKind::Convert { a }
        | IrExprKind::Resize { a }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::BitStreamCast { a, .. }
        | IrExprKind::Stream { value: a, .. }
        | IrExprKind::PartSel { base: a, .. } => repeatable_operand(a),
        IrExprKind::Bin { a, b, .. } | IrExprKind::BitSel { base: a, idx: b } => {
            repeatable_operand(a) && repeatable_operand(b)
        }
        IrExprKind::IdxPartSel { base, base_idx, .. } => {
            repeatable_operand(base) && repeatable_operand(base_idx)
        }
        IrExprKind::Mux { sel, a, b }
        | IrExprKind::ArrayMux { sel, a, b, .. }
        | IrExprKind::StructMux { sel, a, b, .. } => {
            repeatable_operand(sel) && repeatable_operand(a) && repeatable_operand(b)
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            parts.iter().all(repeatable_operand)
        }
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => matches!(elem_sel, IrElemSel::Whole) && indices.iter().all(repeatable_operand),
        IrExprKind::Mutation(mutation) => {
            matches!(mutation.lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. })
                && repeatable_operand(&mutation.value)
        }
        _ => false,
    }
}
