//! Lexical fixed-array folds; values are exact-width owners in the caller frame.

use super::*;

impl Frame<'_, '_> {
    pub(super) fn fixed_array_reduce(
        &mut self,
        reduction: &IrFixedArrayReduction,
        expr: &IrExpr,
    ) -> Result<Value, String> {
        let source = match &reduction.source {
            IrFixedArrayReductionSource::Value(value) => Some(self.expression(value)?),
            IrFixedArrayReductionSource::Array(_) => None,
        };
        let result = self.reserve(expr.width, expr.signed);
        let (ordinal, ordinal_declaration) = self.loop_variable("uint64_t", "reduction_ordinal");
        let count = reduction
            .element_count()
            .ok_or_else(|| "fixed-array reduction element count overflows".to_owned())?;
        self.line(format!(
            "for ({ordinal_declaration} = 0; {ordinal} < {count}ULL; ++{ordinal}) {{"
        ));
        let item = match (&reduction.source, &source) {
            (IrFixedArrayReductionSource::Array(array), None) => {
                let array_index = *array;
                let array = self.ctx.model.array(array_index);
                let address = if array.sparse() {
                    format!(
                        "llg_fixed_array_peek({}, {ordinal})",
                        self.fixed_array_address(array_index)?
                    )
                } else {
                    array.cell_address(&ordinal)
                };
                if self.sampled_reads {
                    let value = self.reserve(reduction.element_width, reduction.element_signed);
                    self.line(format!("llg_sampled_copy({address}, &{});", value.code));
                    value
                } else {
                    self.value(
                        format!("sv4_clone({address})"),
                        reduction.element_width,
                        reduction.element_signed,
                    )
                }
            }
            (IrFixedArrayReductionSource::Value(_), Some(source)) => {
                let width = reduction.element_width;
                self.value(
                    format!(
                        "sv4_part_select({}, (int64_t)(({count}ULL - {ordinal}) * {width}ULL - 1ULL), (int64_t)(({count}ULL - {ordinal} - 1ULL) * {width}ULL))",
                        source.code,
                    ),
                    width,
                    false,
                )
            }
            _ => return Err("fixed-array reduction source ownership mismatch".to_owned()),
        };
        let item = self.convert(
            item,
            reduction.element_width,
            reduction.element_signed,
            reduction.element_two_state,
            false,
        );
        let direction = if reduction.left <= reduction.right {
            "+"
        } else {
            "-"
        };
        let index = self.value(
            format!(
                "sv4_from_u64((uint64_t)({}LL {direction} (int64_t){ordinal}), 32, 1)",
                reduction.left,
            ),
            32,
            true,
        );
        let mut bindings = HashMap::new();
        for (name, value, two_state) in [
            (&reduction.item_name, &item, reduction.element_two_state),
            (&reduction.index_name, &index, true),
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
        let mapped = self.expression(&reduction.value);
        self.bindings.pop();
        let mapped = mapped?;
        let mapped = self.convert(mapped, expr.width, expr.signed, false, false);
        // Seeding with an arithmetic identity would turn a singleton Z into X.
        self.line(format!("if ({ordinal} == 0) {{"));
        self.line(format!("sv4_assign(&{}, {});", result.code, mapped.code));
        self.line("} else {");
        let operation = match reduction.operation {
            IrContainerReduction::Sum => "sv4_add",
            IrContainerReduction::Product => "sv4_mul",
            IrContainerReduction::BitAnd => "sv4_and",
            IrContainerReduction::BitOr => "sv4_or",
            IrContainerReduction::BitXor => "sv4_xor",
        };
        self.assign(
            &format!("&{}", result.code),
            &format!("{operation}({}, {})", result.code, mapped.code),
        );
        self.line("}");
        self.discard(mapped);
        self.discard(index);
        self.discard(item);
        self.line("}");
        if let Some(source) = source {
            self.discard(source);
        }
        Ok(result)
    }
}
