//! Shared validation for descriptor operands and selected views.
use super::*;

impl Validator<'_> {
    pub(super) fn validate_memory_view(
        &self,
        view: &IrMemoryView,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let Some(array) = self.model.arrays.get(view.array) else {
            return self.fail(path, "memory task array index is out of bounds");
        };
        if array.real {
            return self.fail(path, "memory task does not support real arrays");
        }
        if view.dims.is_empty() || view.dims.len() != view.strides.len() {
            return self.fail(path, "memory task requires a fixed packed memory view");
        }
        if view.total == 0 || view.origin >= array.total {
            return self.fail(path, "memory task view has an invalid extent");
        }
        let Some(prefix) = array.dims.len().checked_sub(view.dims.len()) else {
            return self.fail(path, "memory task view rank exceeds its array rank");
        };
        let mut source_strides = vec![1u64; array.dims.len()];
        let mut source_total = 1u64;
        for dimension in (0..array.dims.len()).rev() {
            source_strides[dimension] = source_total;
            let (left, right) = array.dims[dimension];
            let Some(extent) = (i64::from(left) - i64::from(right))
                .unsigned_abs()
                .checked_add(1)
            else {
                return self.fail(path, "memory source dimension overflows");
            };
            let Some(total) = source_total.checked_mul(extent) else {
                return self.fail(path, "memory source extent overflows");
            };
            source_total = total;
        }
        if source_total != array.total {
            return self.fail(path, "memory source dimensions disagree with its storage");
        }
        let mut previous_selector = None;
        let mut selector_offset = 0u64;
        for (index, selector) in view.selectors.iter().enumerate() {
            if selector.dimension >= prefix
                || previous_selector.is_some_and(|previous| previous >= selector.dimension)
            {
                return self.fail(path, "memory selector dimension is invalid or unordered");
            }
            previous_selector = Some(selector.dimension);
            if array.dims[selector.dimension] != (selector.left, selector.right)
                || source_strides[selector.dimension] != selector.stride
            {
                return self.fail(
                    path,
                    "memory selector descriptor disagrees with its source array",
                );
            }
            self.validate_expr(
                &selector.value,
                formals,
                &format!("{path}.selector[{index}]"),
            )?;
            if selector.value.is_real() || selector.value.width == 0 {
                return self.fail(
                    format!("{path}.selector[{index}]"),
                    "memory selector must be a packed integer",
                );
            }
            let (left, right) = array.dims[selector.dimension];
            let extent = (i64::from(left) - i64::from(right)).unsigned_abs();
            let Some(offset) = extent.checked_mul(selector.stride) else {
                return self.fail(path, "memory selector range overflows");
            };
            let Some(total) = selector_offset.checked_add(offset) else {
                return self.fail(path, "memory selector offset overflows");
            };
            selector_offset = total;
        }
        let mut expected_total = 1u64;
        for (offset, dimension) in view.dims.iter().enumerate() {
            let Some(expected) = array.dims.get(prefix + offset) else {
                return self.fail(path, "memory task view dimension is out of bounds");
            };
            let dimension_matches = if view.sliced && offset == 0 {
                let (source_left, source_right) = *expected;
                let (view_left, view_right) = *dimension;
                let within = view_left >= source_left.min(source_right)
                    && view_left <= source_left.max(source_right)
                    && view_right >= source_left.min(source_right)
                    && view_right <= source_left.max(source_right);
                let direction_matches = view_left == view_right
                    || (view_left >= view_right) == (source_left >= source_right);
                within && direction_matches
            } else {
                expected == dimension
            };
            if !dimension_matches || source_strides[prefix + offset] != view.strides[offset] {
                return self.fail(path, "memory task view bounds disagree with its array");
            }
            let Some(extent) = (i64::from(dimension.0) - i64::from(dimension.1))
                .unsigned_abs()
                .checked_add(1)
            else {
                return self.fail(path, "memory task view dimension overflows");
            };
            let Some(total) = expected_total.checked_mul(extent) else {
                return self.fail(path, "memory task view extent overflows");
            };
            expected_total = total;
        }
        if expected_total != view.total {
            return self.fail(path, "memory task view total disagrees with its bounds");
        }
        let Some(last) = view.strides.iter().zip(&view.dims).try_fold(
            view.origin,
            |offset, (stride, (left, right))| {
                // The last element lies `distance` strides past the first.
                let distance = (i64::from(*left) - i64::from(*right)).unsigned_abs();
                offset.checked_add(stride.checked_mul(distance)?)
            },
        ) else {
            return self.fail(path, "memory task view exceeds its array");
        };
        let Some(last) = last.checked_add(selector_offset) else {
            return self.fail(path, "memory task selected view overflows its source array");
        };
        if last >= array.total {
            return self.fail(path, "memory task view exceeds its array");
        }
        Ok(())
    }
    pub(super) fn validate_fixed_value(
        &self,
        value: &IrFixedValue,
        formals: &[IrFormal],
        path: &str,
    ) -> Result<u64, IrValidationError> {
        match value {
            IrFixedValue::Array(view) => {
                self.validate_fixed_activation(view.array, path)?;
                self.validate_memory_view(view, formals, path)?;
                let array = &self.model.arrays[view.array];
                if !array.sparse() {
                    return self.fail(path, "fixed value requires descriptor storage");
                }
                view.total
                    .checked_mul(u64::from(array.elem_width))
                    .ok_or_else(|| IrValidationError::new(path, "fixed operand width overflow"))
            }
            IrFixedValue::Call { array, call } => {
                let result = self
                    .model
                    .arrays
                    .get(*array)
                    .ok_or_else(|| IrValidationError::new(path, "invalid fixed call result"))?;
                if !result.activation || !result.sparse() || !call.args.iter().any(|argument| matches!(argument, IrCallArg::FixedArray(index) if index == array)) {
                    return self.fail(path, "fixed call requires an owned result operand");
                }
                self.fixed_activations
                    .borrow_mut()
                    .push(HashSet::from([*array]));
                let valid = self.validate_stmt(&IrStmt::Call(*call.clone()), formals, path);
                self.fixed_activations.borrow_mut().pop();
                valid?;
                result
                    .total
                    .checked_mul(u64::from(result.elem_width))
                    .ok_or_else(|| IrValidationError::new(path, "fixed call width overflow"))
            }
            IrFixedValue::Conditional {
                selector,
                left,
                right,
                element_cells,
            } => {
                self.validate_expr(selector, formals, path)?;
                if selector.width != 1
                    || selector.signed
                    || selector.fill.is_some()
                    || *element_cells == 0
                {
                    return self.fail(path, "invalid fixed conditional selector or element extent");
                }
                let left = self.validate_fixed_value(left, formals, path)?;
                let right = self.validate_fixed_value(right, formals, path)?;
                if left != right {
                    return self.fail(path, "fixed conditional arms disagree");
                }
                Ok(left)
            }
            IrFixedValue::Stream { parts, .. } => {
                if parts.is_empty() {
                    return self.fail(path, "empty fixed stream");
                }
                parts.iter().try_fold(0u64, |sum, part| {
                    sum.checked_add(self.validate_fixed_value(part, formals, path)?)
                        .ok_or_else(|| IrValidationError::new(path, "fixed stream width overflow"))
                })
            }
            IrFixedValue::Convert { value, array } => {
                let source = self.validate_fixed_value(value, formals, path)?;
                let result = self
                    .model
                    .arrays
                    .get(*array)
                    .ok_or_else(|| IrValidationError::new(path, "invalid fixed cast shape"))?;
                let width = result
                    .total
                    .checked_mul(u64::from(result.elem_width))
                    .ok_or_else(|| IrValidationError::new(path, "fixed cast width overflow"))?;
                // Fixed bit-stream casts require equal sizes (SV 6.24.3).
                if !result.activation || !result.sparse() || width != source {
                    return self.fail(path, "fixed cast requires an equal-size lexical shape");
                }
                Ok(width)
            }
        }
    }
}
