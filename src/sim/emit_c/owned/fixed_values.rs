//! Owned fixed descriptor evaluation and selected view capture.
use super::*;
use crate::sim::ir::IrFixedValue;

impl Frame<'_, '_> {
    pub(super) fn fixed_view(&mut self, view: &IrMemoryView) -> Result<String, String> {
        let owner = self.fixed_array_address(view.array)?;
        if view.origin == 0
            && view.selectors.is_empty()
            && view.total == self.ctx.model.array(view.array).total
        {
            return Ok(owner);
        }
        let origin = self.scalar("uint64_t", format!("{}ULL", view.origin));
        for selector in &view.selectors {
            let value = self.expression(&selector.value)?;
            let index = self.declare("int64_t", "fixed_index", "0".to_owned());
            let (left, right) = (selector.left, selector.right);
            let offset = if left >= right {
                format!("(uint64_t)({left}LL - {index})")
            } else {
                format!("(uint64_t)({index} - {left}LL)")
            };
            self.line(format!("if ({origin} != UINT64_MAX) {{ if (!sv4_to_index_i64({}, &{index}) || {index} < {}LL || {index} > {}LL || {offset} > (UINT64_MAX - {origin}) / {}ULL) {{ {origin} = UINT64_MAX; }} else {{ {origin} += {offset} * {}ULL; }} }}", value.code, left.min(right), left.max(right), selector.stride, selector.stride));
            self.discard(value);
        }
        let pointer = self.scalar("llg_fixed_array_t*", "(llg_fixed_array_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy))".to_owned());
        self.line(format!(
            "llg_fixed_array_view_init({pointer}, {owner}, {origin}, {}ULL, {});",
            view.total,
            u8::from(self.ctx.model.array(view.array).two_state)
        ));
        Ok(pointer)
    }

    pub(super) fn fixed_value_storage(&self, value: &IrFixedValue) -> Result<(usize, u64), String> {
        match value {
            IrFixedValue::Array(view) => Ok((view.array, view.total)),
            IrFixedValue::Call { array, .. } => Ok((*array, self.ctx.model.array(*array).total)),
            IrFixedValue::Conditional { left, .. } => self.fixed_value_storage(left),
            IrFixedValue::Stream { parts, .. } => {
                self.fixed_value_storage(parts.first().ok_or("empty fixed stream")?)
            }
            IrFixedValue::Convert { array, .. } => Ok((*array, self.ctx.model.array(*array).total)),
        }
    }

    pub(super) fn fixed_value(
        &mut self,
        value: &IrFixedValue,
        shape: usize,
        total: u64,
    ) -> Result<String, String> {
        match value {
            IrFixedValue::Array(view) => {
                let source = self.fixed_view(view)?;
                let snapshot = self.new_fixed_array_total(view.array, view.total)?;
                self.line(format!(
                    "llg_fixed_array_copy({snapshot}, {source}, {}, 0);",
                    u8::from(self.ctx.model.array(view.array).two_state)
                ));
                Ok(snapshot)
            }
            IrFixedValue::Call { array, call } => {
                let result = self.new_fixed_array(*array)?;
                self.fixed_arrays.insert(*array, result.clone());
                self.call_statement(call)?;
                Ok(result)
            }
            IrFixedValue::Conditional {
                selector,
                left,
                right,
                element_cells,
            } => {
                // Each arm is emitted once: a known selector evaluates one arm,
                // an ambiguous one evaluates both and merges elements (§11.4.11).
                let two_state = u8::from(self.ctx.model.array(shape).two_state);
                let result = self.new_fixed_array_total(shape, total)?;
                let selector = self.expression(selector)?;
                let state = self.scalar("int", format!("llg_sv4_state({}, 0)", selector.code));
                self.discard(selector);
                self.line(format!("if ({state} != 0) {{"));
                let left_value = self.fixed_value(left, shape, total)?;
                self.line(format!(
                    "llg_fixed_array_copy({result}, {left_value}, {two_state}, 0);"
                ));
                self.line("}");
                self.line(format!("if ({state} != 1) {{"));
                let right_value = self.fixed_value(right, shape, total)?;
                self.line(format!(
                    "if ({state} == 0) llg_fixed_array_copy({result}, {right_value}, {two_state}, 0); \
                     else llg_fixed_array_merge({result}, {result}, {right_value}, {element_cells}ULL, {two_state});"
                ));
                self.line("}");
                Ok(result)
            }
            IrFixedValue::Stream { parts, slice } => {
                let mut sources = Vec::new();
                for part in parts {
                    // Each part keeps its own extent; only the stream result
                    // takes the destination shape.
                    let (part_shape, part_total) = self.fixed_value_storage(part)?;
                    sources.push(self.fixed_value(part, part_shape, part_total)?);
                }
                let result = self.new_fixed_array_total(shape, total)?;
                let sources = self.declare_array_init(
                    "const llg_fixed_array_t*",
                    "fixed_sources",
                    sources.len(),
                    &sources.join(", "),
                );
                self.line(format!(
                    "llg_fixed_array_stream_segments({result}, {sources}, {}, {}, 0, {slice}u);",
                    parts.len(),
                    u8::from(self.ctx.model.array(shape).two_state)
                ));
                Ok(result)
            }
            IrFixedValue::Convert { value, array } => {
                // The source keeps its own shape; the copy reshapes the bit
                // stream into the cast type and clears X/Z for a two-state
                // element domain before any consumer observes the cells.
                let (source_shape, source_total) = self.fixed_value_storage(value)?;
                let source = self.fixed_value(value, source_shape, source_total)?;
                let result = self.new_fixed_array(*array)?;
                self.line(format!(
                    "llg_fixed_array_copy({result}, {source}, {}, 0);",
                    u8::from(self.ctx.model.array(*array).two_state)
                ));
                Ok(result)
            }
        }
    }
}
