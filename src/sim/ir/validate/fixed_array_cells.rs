//! Shape and lexical-binding checks for cell-wise fixed-array operations.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_fixed_array_cells(
        &self,
        cells: &IrFixedArrayCells,
        formals: &[IrFormal],
        path: &str,
        allow_real: bool,
    ) -> Result<FixedArrayCellShape, IrValidationError> {
        self.validate_fixed_activation(cells.array, path)?;
        let array = self.model.arrays.get(cells.array).ok_or_else(|| {
            IrValidationError::new(format!("{path}.array"), "array index is out of bounds")
        })?;
        if (array.real && !allow_real) || cells.prefix.len() >= array.dims.len() {
            return Err(IrValidationError::new(
                path,
                "fixed-array cell view has an unsupported storage shape",
            ));
        }
        for (index, selector) in cells.prefix.iter().enumerate() {
            let selector_path = format!("{path}.prefix[{index}]");
            self.validate_expr(selector, formals, &selector_path)?;
            if selector.is_real() {
                return self.fail(selector_path, "fixed-array selector must be integral");
            }
        }
        cells
            .shape(self.model)
            .ok_or_else(|| IrValidationError::new(path, "fixed-array cell view extent overflows"))
    }

    pub(super) fn validate_fixed_array_order(
        &self,
        order: &IrFixedArrayOrder,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let shape = self.validate_fixed_array_cells(&order.cells, formals, path, false)?;
        let array = &self.model.arrays[order.cells.array];
        if array.is_net() {
            return self.fail(path, "fixed-array ordering cannot write resolved net cells");
        }
        if order.method == IrFixedArrayOrderMethod::Reverse {
            if order.key.is_some() {
                return self.fail(path, "fixed-array reverse has no comparison key");
            }
            return Ok(());
        }
        let item_width = shape
            .element_cells
            .checked_mul(u64::from(array.elem_width))
            .and_then(|width| u32::try_from(width).ok())
            .ok_or_else(|| IrValidationError::new(path, "fixed-array sort item is too wide"))?;
        self.validate_width(item_width, &format!("{path}.item_width"))?;
        self.validate_width(32, &format!("{path}.index_width"))?;
        if order.item_name.is_empty()
            || order.index_name.is_empty()
            || order.item_name == order.index_name
        {
            return self.fail(
                path,
                "fixed-array sort needs distinct nonempty iterator bindings",
            );
        }
        let Some(key) = &order.key else {
            return Ok(());
        };
        let saved_depth = self.reduction_bindings.borrow().len();
        self.reduction_bindings.borrow_mut().extend([
            (order.item_name.clone(), item_width, order.item_signed),
            (order.index_name.clone(), 32, true),
        ]);
        let result = self.validate_expr(key, formals, &format!("{path}.key"));
        self.reduction_bindings.borrow_mut().truncate(saved_depth);
        result?;
        if key.is_real() || key.fill.is_some() {
            return self.fail(path, "fixed-array sort key must be a sized integral value");
        }
        Ok(())
    }
}
