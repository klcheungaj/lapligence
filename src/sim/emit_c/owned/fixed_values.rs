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
        let origin = self.fixed_view_origin(view)?;
        let pointer = self.scalar("llg_fixed_array_t*", "(llg_fixed_array_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy))".to_owned());
        self.line(format!(
            "llg_fixed_array_view_init({pointer}, {owner}, {origin}, {}ULL, {});",
            view.total,
            u8::from(self.ctx.model.array(view.array).two_state)
        ));
        Ok(pointer)
    }

    /// The view's first storage offset, `UINT64_MAX` when any selector is
    /// unknown or outside its dimension. Each selector is evaluated once.
    fn fixed_view_origin(&mut self, view: &IrMemoryView) -> Result<String, String> {
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
        Ok(origin)
    }

    pub(super) fn fixed_value_storage(&self, value: &IrFixedValue) -> Result<(usize, u64), String> {
        match value {
            IrFixedValue::Array(view) => Ok((view.array, view.total)),
            IrFixedValue::Call { array, .. } => Ok((*array, self.ctx.model.array(*array).total)),
            IrFixedValue::Conditional { left, .. } => self.fixed_value_storage(left),
            IrFixedValue::Stream { parts, .. } => parts
                .iter()
                .find_map(|part| self.fixed_value_storage(part).ok())
                .ok_or_else(|| "fixed stream has no descriptor-shaped operand".to_owned()),
            IrFixedValue::Convert { array, .. } => Ok((*array, self.ctx.model.array(*array).total)),
            IrFixedValue::Dense(view) => Ok((view.array, view.total)),
            IrFixedValue::Packed { .. } | IrFixedValue::Selected { .. } => {
                Err("stream operand has no descriptor shape of its own".to_owned())
            }
        }
    }

    /// A zeroed scratch descriptor owned by the current value scope.
    fn fixed_scratch(&mut self) -> String {
        self.scalar("llg_fixed_array_t*", "(llg_fixed_array_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy))".to_owned())
    }

    /// Evaluate a descriptor stream's operands in order into a source list.
    /// An operand with no bits (an empty runtime selection) is skipped, so
    /// the list's length is the returned runtime count.
    fn fixed_stream_sources(&mut self, parts: &[IrFixedValue]) -> Result<(String, String), String> {
        let sources = self.declare_array_init(
            "const llg_fixed_array_t*",
            "fixed_sources",
            parts.len(),
            &vec!["NULL"; parts.len()].join(", "),
        );
        let count = self.scalar("size_t", "0".to_owned());
        for part in parts {
            match part {
                IrFixedValue::Packed {
                    value, cell_width, ..
                } => {
                    let value = self.expression(value)?;
                    let object = self.fixed_scratch();
                    self.line(format!(
                        "if (llg_fixed_array_packed_source({object}, {}, {cell_width}u)) {sources}[{count}++] = {object};",
                        value.code
                    ));
                    self.discard(value);
                }
                IrFixedValue::Selected { array, selector } => {
                    let info = self.ctx.model.array(*array).clone();
                    let (left, right) = info
                        .dims
                        .first()
                        .copied()
                        .ok_or("selected fixed operand has no dimensions")?;
                    let fallback = self.expression(&IrExpr::new(
                        IrExprKind::Const(info.element_fallback()),
                        info.elem_width,
                        false,
                        None,
                    ))?;
                    let owner = self.fixed_array_address(*array)?;
                    let mut owners = Vec::new();
                    let (kind, first, second) =
                        super::containers::stream_selector(self, &mut owners, Some(selector))?;
                    let object = self.fixed_scratch();
                    self.line(format!(
                        "if (llg_fixed_array_with_source({object}, {owner}, {left}LL, {right}LL, {}, {kind}, {first}, {second})) {sources}[{count}++] = {object};",
                        fallback.code
                    ));
                    self.discard(fallback);
                    for value in owners {
                        self.discard(value);
                    }
                }
                IrFixedValue::Stream { parts, slice } => {
                    // A nested stream is imaged lazily with its own reordering.
                    let (inner, inner_count) = self.fixed_stream_sources(parts)?;
                    let object = self.fixed_scratch();
                    self.line(format!(
                        "if (llg_fixed_array_stream_value({object}, {inner}, {inner_count}, {slice}u)) {sources}[{count}++] = {object};"
                    ));
                }
                part => {
                    // Each part keeps its own extent; only the stream result
                    // takes the destination shape.
                    let (shape, total) = self.fixed_value_storage(part)?;
                    let value = self.fixed_value(part, shape, total)?;
                    self.line(format!("{sources}[{count}++] = {value};"));
                }
            }
        }
        Ok((sources, count))
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
                let (sources, count) = self.fixed_stream_sources(parts)?;
                let result = self.new_fixed_array_total(shape, total)?;
                self.line(format!(
                    "llg_fixed_array_stream_segments({result}, {sources}, {count}, {}, 0, {slice}u);",
                    u8::from(self.ctx.model.array(shape).two_state)
                ));
                Ok(result)
            }
            IrFixedValue::Packed {
                value, cell_width, ..
            } => {
                // A whole operand (a dense pattern item) always has its
                // declared, nonzero width.
                let value = self.expression(value)?;
                let object = self.fixed_scratch();
                self.line(format!(
                    "(void)llg_fixed_array_packed_source({object}, {}, {cell_width}u);",
                    value.code
                ));
                self.discard(value);
                Ok(object)
            }
            IrFixedValue::Selected { .. } => {
                Err("a runtime `with` selection is only a descriptor stream operand".to_owned())
            }
            IrFixedValue::Dense(view) => {
                let info = self.ctx.model.array(view.array).clone();
                let origin = self.fixed_view_origin(view)?;
                let fallback = self.expression(&IrExpr::new(
                    IrExprKind::Const(info.element_fallback()),
                    info.elem_width,
                    false,
                    None,
                ))?;
                let object = self.fixed_scratch();
                self.line(format!(
                    "llg_fixed_array_dense_source({object}, {}, {origin}, {}ULL, {});",
                    info.c_name, view.total, fallback.code
                ));
                self.discard(fallback);
                Ok(object)
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
