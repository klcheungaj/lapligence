//! Shape and reference checks for fixed-array method folds.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_fixed_array_reduction(
        &self,
        reduction: &IrFixedArrayReduction,
        expr: &IrExpr,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.validate_width(reduction.element_width, &format!("{path}.element_width"))?;
        self.validate_width(expr.width, path)?;
        // The emitter materializes the signed index even when the map does not
        // read it. Include that intermediate in capacity accounting.
        self.validate_width(32, &format!("{path}.index_width"))?;
        // A fill literal may be the map, but it is materialized at the map's
        // self-determined width. The reduction result itself is never a fill.
        if expr.fill.is_some()
            || (expr.width, expr.signed) != (reduction.value.width, reduction.value.signed)
        {
            return self.fail(
                path,
                "fixed-array reduction result disagrees with its integral map type",
            );
        }
        if reduction.item_name.is_empty()
            || reduction.index_name.is_empty()
            || reduction.item_name == reduction.index_name
        {
            return self.fail(
                path,
                "fixed-array reduction needs distinct nonempty iterator bindings",
            );
        }
        match &reduction.source {
            IrFixedArrayReductionSource::Array(index) => {
                let array = self.model.arrays.get(*index).ok_or_else(|| {
                    IrValidationError::new(format!("{path}.array"), "array index is out of bounds")
                })?;
                if array.total > crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS {
                    return self.fail(
                        path,
                        format!(
                            "fixed-array reduction source has {} cells; selected cell-wise storage limit is {} cells",
                            array.total,
                            crate::sim::ir::LLG_MAX_FIXED_ARRAY_CELLS
                        ),
                    );
                }
                if array.real
                    || array.dims.len() != 1
                    || reduction.element_count() != Some(array.total)
                    || array.elem_width != reduction.element_width
                {
                    return self.fail(path, "fixed-array reduction source storage shape mismatch");
                }
            }
            IrFixedArrayReductionSource::Cells(cells) => {
                let shape = self.validate_fixed_array_cells(
                    cells,
                    formals,
                    &format!("{path}.cells"),
                    false,
                )?;
                let array = &self.model.arrays[cells.array];
                if shape.element_cells != 1
                    || (shape.left, shape.right) != (reduction.left, reduction.right)
                    || array.elem_width != reduction.element_width
                {
                    return self.fail(path, "fixed-array reduction source storage shape mismatch");
                }
            }
            IrFixedArrayReductionSource::Value(source) => {
                self.validate_expr(source, formals, &format!("{path}.source"))?;
                if source.is_real()
                    || source.fill.is_some()
                    || reduction
                        .element_count()
                        .and_then(|count| count.checked_mul(u64::from(reduction.element_width)))
                        != Some(u64::from(source.width))
                {
                    return self.fail(
                        path,
                        "fixed-array reduction payload does not contain the declared elements",
                    );
                }
            }
        }
        let saved_depth = self.reduction_bindings.borrow().len();
        self.reduction_bindings.borrow_mut().extend([
            (
                reduction.item_name.clone(),
                reduction.element_width,
                reduction.element_signed,
            ),
            (reduction.index_name.clone(), 32, true),
        ]);
        let result = self.validate_expr(&reduction.value, formals, &format!("{path}.value"));
        self.reduction_bindings.borrow_mut().truncate(saved_depth);
        result
    }
}
