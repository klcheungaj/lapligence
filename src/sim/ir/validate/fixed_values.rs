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
        self.validate_memory_geometry(
            MemoryGeometry {
                source_dims: &array.dims,
                source_total: array.total,
                origin: view.origin,
                selectors: &view.selectors,
                sliced: view.sliced,
                dims: &view.dims,
                strides: &view.strides,
                total: view.total,
            },
            formals,
            path,
        )
    }

    /// Check that a selected view lies inside its source dimensions; shared
    /// by model-array views and memory-task images.
    pub(super) fn validate_memory_geometry(
        &self,
        view: MemoryGeometry<'_>,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let array = MemorySource {
            dims: view.source_dims,
            total: view.source_total,
        };
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
            if let crate::sim::ir::IrMemorySelectorKind::SliceBase { width, .. } = selector.kind {
                // The variable position of the view's slice: the last
                // selector, on the first view dimension, whose placeholder
                // bounds have the slice's extent.
                let extent = view
                    .dims
                    .first()
                    .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1);
                if index + 1 != view.selectors.len()
                    || !view.sliced
                    || selector.dimension != prefix
                    || array.dims.get(selector.dimension) != Some(&(selector.left, selector.right))
                    || source_strides.get(selector.dimension) != Some(&selector.stride)
                    || extent != Some(u64::from(width))
                {
                    return self.fail(path, "memory slice selector disagrees with its view");
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
                continue;
            }
            if !selector.is_index() {
                return self.fail(
                    path,
                    "a fixed memory view has no runtime range slice or container row",
                );
            }
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
        let Some(last) = view.strides.iter().zip(view.dims).try_fold(
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
    /// A load's image root must be a packed lvalue and a dump's a packed
    /// value, each exactly as wide as the root's cells.
    pub(super) fn validate_memory_image_root(
        &self,
        task: &crate::sim::ir::IrMemoryAccess,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        use crate::sim::ir::{IrMemoryImage, IrMemoryStorage};
        let IrMemoryStorage::Image(image) = &task.storage else {
            return Ok(());
        };
        let width = task
            .cells()
            .and_then(|cells| cells.checked_mul(u64::from(task.element_width)));
        let root_width = match image {
            IrMemoryImage::Target(lhs) => {
                if task.write {
                    return self.fail(path, "a memory dump has no image target");
                }
                self.validate_lhs(lhs, formals, &format!("{path}.image"))?;
                self.lhs_packed_width(lhs)
            }
            IrMemoryImage::Value(value) => {
                if !task.write {
                    return self.fail(path, "a memory load needs an image target");
                }
                self.validate_expr(value, formals, &format!("{path}.image"))?;
                (!value.is_real()).then_some(value.width)
            }
        };
        if root_width.map(u64::from) != width {
            return self.fail(path, "memory image root width disagrees with its cells");
        }
        Ok(())
    }

    /// Storage-specific shape checks of a memory task outside model arrays.
    pub(super) fn validate_memory_access_storage(
        &self,
        task: &crate::sim::ir::IrMemoryAccess,
        path: &str,
    ) -> ValidationResult {
        if task.element_width == 0 || task.element_width > LLG_MAX_WIDTH {
            return self.fail(path, "memory element width is outside packed capacity");
        }
        let cells = task.cells();
        if !task.root_dims.is_empty() && cells.is_none() {
            return self.fail(path, "memory root cell count overflows");
        }
        match &task.storage {
            crate::sim::ir::IrMemoryStorage::Image(_) => {
                if cells
                    .and_then(|cells| cells.checked_mul(u64::from(task.element_width)))
                    .is_none_or(|width| width > u64::from(LLG_MAX_WIDTH))
                {
                    return self.fail(path, "memory image width is outside packed capacity");
                }
            }
            crate::sim::ir::IrMemoryStorage::Container(index) => {
                let Some(container) = self.model.containers.get(*index) else {
                    return self.fail(path, "memory container index is out of bounds");
                };
                if let IrContainerElement::FixedArray {
                    dimensions,
                    element,
                } = &container.element
                {
                    return self.validate_memory_rows(task, container, dimensions, element, path);
                }
                if task
                    .selectors
                    .iter()
                    .any(|selector| selector.kind == crate::sim::ir::IrMemorySelectorKind::Row)
                {
                    return self.fail(path, "a memory container row needs fixed-array elements");
                }
                if container.element
                    != (IrContainerElement::Packed {
                        width: task.element_width,
                        signed: task.signed,
                        two_state: task.two_state,
                    })
                {
                    return self.fail(path, "memory container element disagrees with the task");
                }
                match (&container.kind, cells) {
                    (IrContainerKind::Dynamic, Some(cells))
                        if container.initial_size == Some(cells) => {}
                    (IrContainerKind::Dynamic | IrContainerKind::Queue { .. }, None)
                        if task.selectors.is_empty()
                            && !task.sliced
                            && task.dims.is_empty()
                            && task.strides.is_empty()
                            && task.origin == 0
                            && task.total == 0 => {}
                    // A whole integral-index associative array.
                    (
                        IrContainerKind::Associative {
                            key: crate::sim::ir::IrAssocKey::Integral { .. },
                        },
                        None,
                    ) if task.root_dims.is_empty()
                        && task.selectors.is_empty()
                        && !task.sliced
                        && task.dims.is_empty()
                        && task.strides.is_empty()
                        && task.origin == 0
                        && task.total == 0 => {}
                    // A slice whose position or bounds resolve when the
                    // task runs: an indexed slice of either kind, or the two
                    // bounds of a queue range slice. The runtime checks the
                    // resolved slice against the current size.
                    (IrContainerKind::Dynamic | IrContainerKind::Queue { .. }, None)
                        if task.sliced
                            && task.root_dims.is_empty()
                            && task.strides == [1]
                            && task.origin == 0
                            && self.resizable_slice_selectors(task) => {}
                    // A constant ascending slice; the runtime checks it
                    // against the current size.
                    (IrContainerKind::Dynamic | IrContainerKind::Queue { .. }, None)
                        if task.selectors.is_empty()
                            && task.sliced
                            && task.strides == [1]
                            && matches!(task.dims.as_slice(), [(low, high)]
                                if 0 <= *low
                                    && low <= high
                                    && task.origin == u64::from(low.unsigned_abs())
                                    && task.total == u64::from(high.abs_diff(*low)) + 1) => {}
                    _ => {
                        return self.fail(
                            path,
                            "memory container must be a fixed-size class property, dynamic array, queue or integral-index associative array",
                        )
                    }
                }
            }
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
                let valid =
                    self.validate_stmt(&IrStmt::Call(Box::new(*call.clone())), formals, path);
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
                cell_leaves,
            } => {
                self.validate_expr(selector, formals, path)?;
                if selector.width != 1 || selector.signed || selector.fill.is_some() {
                    return self.fail(path, "invalid fixed conditional selector");
                }
                let left = self.validate_fixed_value(left, formals, path)?;
                let right = self.validate_fixed_value(right, formals, path)?;
                if left != right {
                    return self.fail(path, "fixed conditional arms disagree");
                }
                if let Some(leaves) = cell_leaves {
                    // The runtime checks that the period is the cell width.
                    let cell = leaves.default.width;
                    self.validate_leaf_merge(leaves, cell, &format!("{path}.cell_leaves"))?;
                    if cell == 0 || left % u64::from(cell) != 0 {
                        return self.fail(
                            path,
                            "fixed conditional leaf period does not divide the value",
                        );
                    }
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
            IrFixedValue::Dense(view) => {
                self.validate_memory_view(view, formals, path)?;
                let array = &self.model.arrays[view.array];
                if array.sparse() || array.real || array.is_net() || array.elem_width == 0 {
                    return self.fail(path, "dense fixed operand requires dense integral storage");
                }
                view.total
                    .checked_mul(u64::from(array.elem_width))
                    .ok_or_else(|| IrValidationError::new(path, "fixed operand width overflow"))
            }
            IrFixedValue::Packed {
                value,
                cell_width,
                runtime_sized,
            } => {
                self.validate_expr(value, formals, path)?;
                if value.is_real()
                    || *cell_width == 0
                    || (!*runtime_sized && !value.width.is_multiple_of(*cell_width))
                {
                    return self.fail(path, "packed fixed operand requires whole packed cells");
                }
                // A runtime-sized width is checked when the stream is built.
                Ok(if *runtime_sized {
                    0
                } else {
                    u64::from(value.width)
                })
            }
            IrFixedValue::BitStream {
                stream,
                cell_width,
                consume,
            } => {
                stream.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                stream.expressions(&mut |value| {
                    if result.is_ok() {
                        result = self.validate_expr(value, formals, path);
                    }
                });
                result?;
                if *cell_width == 0
                    || (stream.unpack.is_some() && consume.is_none())
                    || consume.is_some_and(|bits| bits % u64::from(*cell_width) != 0)
                {
                    return self.fail(path, "bit stream fixed operand requires whole cells");
                }
                // Its width is checked when the stream is built.
                Ok(0)
            }
            IrFixedValue::Selected { array, selector } => {
                self.validate_fixed_activation(*array, path)?;
                let valid = self.model.arrays.get(*array).is_some_and(|array| {
                    array.sparse() && array.dims.len() == 1 && array.elem_width != 0
                });
                if !valid {
                    return self.fail(
                        path,
                        "selected fixed operand requires one-dimensional descriptor storage",
                    );
                }
                validate_stream_selector(selector)
                    .map_err(|error| IrValidationError::new(path, error.detail()))?;
                match selector {
                    IrStreamSelector::Index(index) => self.validate_expr(index, formals, path)?,
                    IrStreamSelector::Range { left, right } => {
                        self.validate_expr(left, formals, path)?;
                        self.validate_expr(right, formals, path)?;
                    }
                    IrStreamSelector::Indexed { base, width, .. } => {
                        self.validate_expr(base, formals, path)?;
                        self.validate_expr(width, formals, path)?;
                    }
                }
                // The selected extent is runtime-valued; the stream checks it.
                Ok(0)
            }
        }
    }
}

impl Validator<'_> {
    /// The runtime slice selectors of a dynamic array or queue view and its
    /// placeholder first dimension: one indexed-slice position with
    /// `[0:width-1]`, or a left and a right queue bound with `[0:0]`.
    fn resizable_slice_selectors(&self, task: &crate::sim::ir::IrMemoryAccess) -> bool {
        use crate::sim::ir::IrMemorySelectorKind as Kind;
        if task.dims.len() != 1 {
            return false;
        }
        let packed = |selector: &crate::sim::ir::IrMemorySelector| {
            selector.dimension == 0
                && selector.left == 0
                && selector.right == 0
                && selector.stride == 1
                && !selector.value.is_real()
                && selector.value.width != 0
        };
        match task.selectors.as_slice() {
            [base] => match base.kind {
                Kind::SliceBase { width, .. } => {
                    packed(base)
                        && i32::try_from(width - 1)
                            .is_ok_and(|span| task.dims.first() == Some(&(0, span)))
                        && u64::from(width) == task.total
                }
                _ => false,
            },
            [left, right] => {
                matches!(left.kind, Kind::SliceLeft)
                    && matches!(right.kind, Kind::SliceRight)
                    && packed(left)
                    && packed(right)
                    && task.dims.first() == Some(&(0, 0))
                    && task.total == 1
            }
            _ => false,
        }
    }

    /// A memory task over one element of a dynamic array, queue or
    /// integral-index associative array whose elements are fixed arrays of
    /// packed words: a leading row selector, with the element's dimensions
    /// as `root_dims` (the whole container is not a memory, decision S27-D22).
    fn validate_memory_rows(
        &self,
        task: &crate::sim::ir::IrMemoryAccess,
        container: &crate::sim::ir::IrContainer,
        dimensions: &[(i32, i32)],
        element: &IrContainerElement,
        path: &str,
    ) -> ValidationResult {
        use crate::sim::ir::IrMemorySelectorKind as Kind;
        let keyed = matches!(
            container.kind,
            IrContainerKind::Associative {
                key: crate::sim::ir::IrAssocKey::Integral { .. }
            } | IrContainerKind::Dynamic
                | IrContainerKind::Queue { .. }
        );
        if !keyed
            || *element
                != (IrContainerElement::Packed {
                    width: task.element_width,
                    signed: task.signed,
                    two_state: task.two_state,
                })
        {
            return self.fail(
                path,
                "memory rows must be packed fixed-array elements of a dynamic array, queue or integral-index associative array",
            );
        }
        let Some(row) = task.selectors.first().filter(|row| row.kind == Kind::Row) else {
            return self.fail(path, "a memory over container rows needs one element");
        };
        // The view inside the element is checked as a fixed geometry.
        if task.root_dims != dimensions
            || row.dimension != 0
            || row.left != 0
            || row.right != 0
            || row.stride != 1
        {
            return self.fail(path, "memory container row disagrees with its element");
        }
        Ok(())
    }
}

/// A memory view and the dimensions of the storage it selects from.
pub(super) struct MemoryGeometry<'a> {
    pub(super) source_dims: &'a [(i32, i32)],
    pub(super) source_total: u64,
    pub(super) origin: u64,
    pub(super) selectors: &'a [crate::sim::ir::IrMemorySelector],
    pub(super) sliced: bool,
    pub(super) dims: &'a [(i32, i32)],
    pub(super) strides: &'a [u64],
    pub(super) total: u64,
}

struct MemorySource<'a> {
    dims: &'a [(i32, i32)],
    total: u64,
}
