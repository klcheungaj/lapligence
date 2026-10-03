//! Owned fixed descriptor evaluation and selected view capture.
use super::*;
use crate::sim::ir::IrFixedValue;

impl Frame<'_, '_> {
    pub(super) fn fixed_view(&mut self, view: &IrMemoryView) -> Result<String, String> {
        let owner = self.fixed_array_address(view.array)?;
        if view.origin == 0 && view.selectors.is_empty() && view.total == self.ctx.model.array(view.array).total { return Ok(owner); }
        let origin = self.scalar("uint64_t", format!("{}ULL", view.origin));
        for selector in &view.selectors {
            let value = self.expression(&selector.value)?;
            let index = self.declare("int64_t", "fixed_index", "0".to_owned());
            let (left, right) = (selector.left, selector.right);
            let offset = if left >= right { format!("(uint64_t)({left}LL - {index})") } else { format!("(uint64_t)({index} - {left}LL)") };
            self.line(format!("if ({origin} != UINT64_MAX) {{ if (!sv4_to_index_i64({}, &{index}) || {index} < {}LL || {index} > {}LL || {offset} > (UINT64_MAX - {origin}) / {}ULL) {{ {origin} = UINT64_MAX; }} else {{ {origin} += {offset} * {}ULL; }} }}", value.code, left.min(right), left.max(right), selector.stride, selector.stride));
            self.discard(value);
        }
        let pointer = self.scalar("llg_fixed_array_t*", "(llg_fixed_array_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy))".to_owned());
        self.line(format!("llg_fixed_array_view_init({pointer}, {owner}, {origin}, {}ULL, {});", view.total, u8::from(self.ctx.model.array(view.array).two_state)));
        Ok(pointer)
    }

    pub(super) fn fixed_value_storage(&self, value: &IrFixedValue) -> Result<(usize, u64), String> {
        match value {
            IrFixedValue::Array(view) => Ok((view.array, view.total)),
            IrFixedValue::Call { array, .. } => Ok((*array, self.ctx.model.array(*array).total)),
            IrFixedValue::Conditional { left, .. } => self.fixed_value_storage(left),
            IrFixedValue::Stream { parts, .. } => self.fixed_value_storage(parts.first().ok_or("empty fixed stream")?),
        }
    }

    pub(super) fn fixed_value(&mut self, value: &IrFixedValue, shape: usize, total: u64) -> Result<String, String> {
        match value {
            IrFixedValue::Array(view) => {
                let source = self.fixed_view(view)?;
                let snapshot = self.new_fixed_array_total(view.array, view.total)?;
                self.line(format!("llg_fixed_array_copy({snapshot}, {source}, {}, 0);", u8::from(self.ctx.model.array(view.array).two_state)));
                Ok(snapshot)
            }
            IrFixedValue::Call { array, call } => {
                let result = self.new_fixed_array(*array)?;
                self.fixed_arrays.insert(*array, result.clone());
                self.call_statement(call)?;
                Ok(result)
            }
            IrFixedValue::Conditional { selector, left, right, element_cells } => {
                let result = self.new_fixed_array_total(shape, total)?;
                let selector = self.expression(selector)?;
                let state = self.scalar("int", format!("llg_sv4_state({}, 0)", selector.code));
                self.discard(selector);
                self.line(format!("if ({state} == 1) {{"));
                let left_value = self.fixed_value(left, shape, total)?;
                self.line(format!("llg_fixed_array_copy({result}, {left_value}, {}, 0);", u8::from(self.ctx.model.array(shape).two_state)));
                self.line(format!("}} else if ({state} == 0) {{"));
                let right_value = self.fixed_value(right, shape, total)?;
                self.line(format!("llg_fixed_array_copy({result}, {right_value}, {}, 0);", u8::from(self.ctx.model.array(shape).two_state)));
                self.line("} else {");
                let left_value = self.fixed_value(left, shape, total)?;
                let right_value = self.fixed_value(right, shape, total)?;
                self.line(format!("llg_fixed_array_merge({result}, {left_value}, {right_value}, {element_cells}ULL, {});", u8::from(self.ctx.model.array(shape).two_state)));
                self.line("}");
                Ok(result)
            }
            IrFixedValue::Stream { parts, slice } => {
                let mut sources = Vec::new();
                for part in parts { sources.push(self.fixed_value(part, shape, total)?); }
                let result = self.new_fixed_array_total(shape, total)?;
                let sources = self.declare_array_init("const llg_fixed_array_t*", "fixed_sources", sources.len(), &sources.join(", "));
                self.line(format!("llg_fixed_array_stream_segments({result}, {sources}, {}, {}, 0, {slice}u);", parts.len(), u8::from(self.ctx.model.array(shape).two_state)));
                Ok(result)
            }
        }
    }
}
