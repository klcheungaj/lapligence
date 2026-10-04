//! Cell-wise fixed-array traversal and in-place ordering.
//!
//! Generated code is one loop per operation, independent of the array extent.
//! Descriptor-backed cells are peeked for reads and materialized only when a
//! write publishes them.

use super::*;

/// Runtime view of the cells selected by an [`IrFixedArrayCells`].
pub(super) struct CellView {
    /// C condition: every selector was known and in range.
    pub valid: String,
    /// C name of the first selected linear cell (zero when invalid).
    pub base: String,
    pub shape: FixedArrayCellShape,
}

impl Frame<'_, '_> {
    /// Evaluate the selectors once, in order, like an ordinary indexed read.
    pub(super) fn fixed_cells_view(
        &mut self,
        cells: &IrFixedArrayCells,
    ) -> Result<CellView, String> {
        let shape = cells
            .shape(self.ctx.model)
            .ok_or_else(|| "fixed-array cell view has an unsupported shape".to_owned())?;
        let dims = self.ctx.model.array(cells.array).dims.clone();
        let mut conditions = Vec::new();
        let mut linear = "0ULL".to_owned();
        for (expr, (left, right)) in cells.prefix.iter().zip(&dims) {
            let value = self.expression(expr)?;
            let number = self.declare("int64_t", "index", "0".to_owned());
            let valid = self.scalar(
                "int",
                format!(
                    "(sv4_to_index_i64({}, &{number}) && {number} >= {}LL && {number} <= {}LL)",
                    value.code,
                    left.min(right),
                    left.max(right)
                ),
            );
            let offset = if left >= right {
                format!("({left}LL - {number})")
            } else {
                format!("({number} - {left}LL)")
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            linear = format!("(({linear}) * {extent}ULL + ({valid} ? (uint64_t){offset} : 0ULL))");
            conditions.push(valid);
            self.discard(value);
        }
        let valid = if conditions.is_empty() {
            "1".to_owned()
        } else {
            self.scalar("int", conditions.join(" && "))
        };
        let base = self.scalar("uint64_t", format!("({linear}) * {}ULL", shape.cells));
        Ok(CellView { valid, base, shape })
    }

    /// Address of one linear cell; a read never materializes descriptor storage.
    pub(super) fn fixed_cell_address(
        &self,
        array: usize,
        linear: &str,
        writable: bool,
    ) -> Result<String, String> {
        let info = self.ctx.model.array(array);
        Ok(if info.sparse() {
            format!(
                "{}({}, {linear})",
                if writable {
                    "llg_fixed_array_cell"
                } else {
                    "llg_fixed_array_peek"
                },
                self.fixed_array_address(array)?
            )
        } else {
            info.cell_address(linear)
        })
    }

    /// Owned copy of one packed cell, or the element's invalid-index default.
    pub(super) fn fixed_cell_read(
        &mut self,
        array: usize,
        valid: &str,
        linear: &str,
    ) -> Result<Value, String> {
        let info = self.ctx.model.array(array);
        let (width, signed) = (info.elem_width, info.signed);
        let default = info
            .element_uninitialized
            .as_ref()
            .map(emit_const)
            .unwrap_or_else(|| {
                super::super::expressions::packed_default(width, signed, info.two_state)
            });
        let address = self.fixed_cell_address(array, linear, false)?;
        if self.sampled_reads {
            let value = self.reserve(width, signed);
            self.line(format!(
                "if ({valid}) {{ llg_sampled_copy({address}, &{}); }} else {{",
                value.code
            ));
            self.assign(&format!("&{}", value.code), &default);
            self.line("}");
            return Ok(value);
        }
        Ok(self.value(
            format!("({valid}) ? sv4_clone({address}) : {default}"),
            width,
            signed,
        ))
    }

    /// Evaluate whether `source` matches any selected cell (SV 11.4.13). A
    /// known match ends the traversal; otherwise an X comparison dominates 0.
    pub(super) fn inside_cells(
        &mut self,
        source: &Value,
        cells: &IrFixedArrayCells,
    ) -> Result<Value, String> {
        let view = self.fixed_cells_view(cells)?;
        let real = self.ctx.model.array(cells.array).real;
        let matched = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
        // An invalid selector reads default elements; one comparison decides.
        let count = format!("(({}) ? {}ULL : 1ULL)", view.valid, view.shape.cells);
        let (ordinal, declaration) = self.loop_variable("uint64_t", "inside_cell");
        self.line(format!(
            "for ({declaration} = 0; {ordinal} < {count} && !{}; ++{ordinal}) {{",
            matched.truth()
        ));
        let linear = format!("({} + {ordinal})", view.base);
        let check = if real {
            let address = self.fixed_cell_address(cells.array, &linear, false)?;
            let item = self.value(format!("({}) ? *({address}) : 0.0", view.valid), 0, false);
            let code = format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), item.real());
            let check = self.value(code, 1, false);
            self.discard(item);
            check
        } else {
            let item = self.fixed_cell_read(cells.array, &view.valid, &linear)?;
            let code = if source.width == 0 {
                format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), item.real())
            } else {
                format!("sv4_wild_eq({}, {})", source.code, item.code)
            };
            self.replace(item, code, 1, false)
        };
        self.assign(
            &format!("&{}", matched.code),
            &format!("sv4_logor({}, {})", matched.code, check.code),
        );
        self.discard(check);
        self.line("}");
        Ok(matched)
    }

    pub(super) fn fixed_array_order(&mut self, order: &IrFixedArrayOrder) -> Result<(), String> {
        if self.read_only_callback {
            return Err(pending(
                "side-effect-capable evaluator expressions: fixed-array ordering writes visible state",
            ));
        }
        let mark = self.cancellation_mark();
        let array = order.cells.array;
        let view = self.fixed_cells_view(&order.cells)?;
        let shape = view.shape;
        let (count, cells) = (shape.count, shape.element_cells);
        let base = view.base.clone();
        let cell =
            |element: &str, cell: &str| format!("({base} + ({element}) * {cells}ULL + {cell})");
        self.line(format!("if ({}) {{", view.valid));
        match order.method {
            IrFixedArrayOrderMethod::Reverse => {
                let (element, element_declaration) =
                    self.loop_variable("uint64_t", "reverse_element");
                let (offset, offset_declaration) = self.loop_variable("uint64_t", "reverse_cell");
                self.line(format!(
                    "for ({element_declaration} = 0; {element} < {}ULL; ++{element}) {{",
                    count / 2
                ));
                self.line(format!(
                    "for ({offset_declaration} = 0; {offset} < {cells}ULL; ++{offset}) {{"
                ));
                let first = cell(&element, &offset);
                let second = cell(&format!("{}ULL - 1ULL - {element}", count), &offset);
                // Two unmaterialized descriptor cells share the default; leave
                // them unmaterialized rather than publishing equal values.
                let first_read = self.fixed_cell_address(array, &first, false)?;
                let second_read = self.fixed_cell_address(array, &second, false)?;
                self.line(format!("if ({first_read} != {second_read}) {{"));
                let first = self.fixed_cell_address(array, &first, true)?;
                let first = self.scalar("sv4_t*", first);
                let second = self.fixed_cell_address(array, &second, true)?;
                let second = self.scalar("sv4_t*", second);
                let info = self.ctx.model.array(array);
                let saved = self.value(format!("sv4_clone({first})"), info.elem_width, info.signed);
                self.line(format!("llg_ba({first}, *{second});"));
                self.line(format!("llg_ba({second}, {});", saved.code));
                self.discard(saved);
                self.line("}");
                self.line("}");
                self.line("}");
            }
            IrFixedArrayOrderMethod::Sort | IrFixedArrayOrderMethod::RSort => {
                self.fixed_array_sort(order, &view)?;
            }
        }
        self.line("}");
        self.cancellation_check_covering(mark)
    }

    fn fixed_array_sort(
        &mut self,
        order: &IrFixedArrayOrder,
        view: &CellView,
    ) -> Result<(), String> {
        let array = order.cells.array;
        let shape = view.shape;
        let (count, cells) = (shape.count, shape.element_cells);
        let base = view.base.clone();
        let info = self.ctx.model.array(array);
        let cell_width = info.elem_width;
        let item_width = u32::try_from(u64::from(cell_width) * cells)
            .map_err(|_| "fixed-array sort item is too wide".to_owned())?;
        let workspace = self.scalar(
            "llg_fixed_order_t*",
            "(llg_fixed_order_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_fixed_order_t), llg_fixed_order_destroy))".to_owned(),
        );
        self.line(format!(
            "llg_fixed_order_init({workspace}, {count}ULL, {cells}ULL);"
        ));

        // Keys: once per element, in declaration order (SV 7.12.2-7.12.4).
        let (element, element_declaration) = self.loop_variable("uint64_t", "sort_element");
        self.line(format!(
            "for ({element_declaration} = 0; {element} < {count}ULL; ++{element}) {{"
        ));
        let first = format!("({base} + {element} * {cells}ULL)");
        let item = self.fixed_cell_read(array, "1", &first)?;
        let item = if cells > 1 {
            let (offset, offset_declaration) = self.loop_variable("uint64_t", "sort_cell");
            self.line(format!(
                "for ({offset_declaration} = 1; {offset} < {cells}ULL; ++{offset}) {{"
            ));
            let address =
                self.fixed_cell_address(array, &format!("({first} + {offset})"), false)?;
            self.assign(
                &format!("&{}", item.code),
                &format!("sv4_concat({}, *({address}))", item.code),
            );
            self.line("}");
            Value {
                width: item_width,
                signed: false,
                ..item
            }
        } else {
            item
        };
        let item = self.convert(
            item,
            item_width,
            order.item_signed,
            order.item_two_state,
            false,
        );
        let (left, right) = (shape.left, shape.right);
        let direction = if left <= right { "+" } else { "-" };
        let index = self.value(
            format!("sv4_from_u64((uint64_t)({left}LL {direction} (int64_t){element}), 32, 1)"),
            32,
            true,
        );
        let key = if let Some(key) = &order.key {
            let mut bindings = HashMap::new();
            for (name, value, two_state) in [
                (&order.item_name, &item, order.item_two_state),
                (&order.index_name, &index, true),
            ] {
                bindings.insert(
                    name.clone(),
                    Binding {
                        address: format!("&{}", value.code),
                        width: value.width,
                        signed: value.signed,
                        two_state,
                        shortreal: false,
                        automatic: true,
                    },
                );
            }
            self.bindings.push(bindings);
            let key = self.expression(key);
            self.bindings.pop();
            let key = self.own(key?);
            self.line(format!(
                "sv4_move(&{workspace}->keys[{element}], &{});",
                key.code
            ));
            self.discard(key);
            self.discard(item);
            None
        } else {
            Some(item)
        };
        if let Some(item) = key {
            self.line(format!(
                "sv4_move(&{workspace}->keys[{element}], &{});",
                item.code
            ));
            self.discard(item);
        }
        self.discard(index);
        self.line("}");

        // Apply the permutation cycle by cycle; each moved cell is published
        // through the ordinary blocking store.
        let (position, position_declaration) = self.loop_variable("uint64_t", "sort_position");
        let hole = self.declare("uint64_t", "sort_hole", "0".to_owned());
        let source = self.declare("uint64_t", "sort_source", "0".to_owned());
        let (offset, offset_declaration) = self.loop_variable("uint64_t", "sort_save");
        let (moved_offset, moved_declaration) = self.loop_variable("uint64_t", "sort_move");
        let cell =
            |element: &str, offset: &str| format!("({base} + {element} * {cells}ULL + {offset})");
        self.line(format!(
            "if (llg_fixed_order_sort({workspace}, {})) {{",
            u8::from(order.method == IrFixedArrayOrderMethod::RSort)
        ));
        self.line(format!(
            "for ({position_declaration} = 0; {position} < {count}ULL; ++{position}) {{"
        ));
        self.line(format!(
            "if ({workspace}->order[{position}] != {position}) {{"
        ));
        self.line(format!(
            "for ({offset_declaration} = 0; {offset} < {cells}ULL; ++{offset}) {{"
        ));
        let saved = self.fixed_cell_address(array, &cell(&position, &offset), false)?;
        self.assign(
            &format!("&{workspace}->row[{offset}]"),
            &format!("sv4_clone({saved})"),
        );
        self.line("}");
        self.line(format!("{hole} = {position};"));
        self.line(format!("while ({workspace}->order[{hole}] != {hole}) {{"));
        self.line(format!("{source} = {workspace}->order[{hole}];"));
        self.line(format!("{workspace}->order[{hole}] = {hole};"));
        self.line(format!(
            "for ({moved_declaration} = 0; {moved_offset} < {cells}ULL; ++{moved_offset}) {{"
        ));
        let target = self.fixed_cell_address(array, &cell(&hole, &moved_offset), true)?;
        let target = self.scalar("sv4_t*", target);
        let moved = self.fixed_cell_address(array, &cell(&source, &moved_offset), false)?;
        self.line(format!(
            "llg_ba({target}, {source} == {position} ? {workspace}->row[{moved_offset}] : *({moved}));"
        ));
        self.line("}");
        self.line(format!("{hole} = {source};"));
        self.line("}");
        self.line("}");
        self.line("}");
        self.line("}");
        Ok(())
    }
}
