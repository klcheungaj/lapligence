//! Streaming.

use super::*;

impl<'a> Codegen<'a> {
    /// Lower the selector attached to a streaming operand.  The array base is
    /// retained by the owning streaming node; only selector bounds become
    /// runtime expressions, so each bound is evaluated once by the C call.
    pub(in super::super) fn lower_stream_selector(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStreamSelector, String> {
        let lower_integral = |this: &mut Self, bound: NodeId| -> Result<IrExpr, String> {
            let value = this.lower_expr(path, bound)?;
            if value.is_real() {
                return Err(format!(
                    "streaming `with` selector bound must be integral in `{path}`"
                ));
            }
            Ok(value)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(IrStreamSelector::Index(lower_integral(self, *index)?))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                Ok(IrStreamSelector::Range {
                    left: lower_integral(self, *left)?,
                    right: lower_integral(self, *right)?,
                })
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => Ok(IrStreamSelector::Indexed {
                base: lower_integral(self, *base_expr)?,
                width: lower_integral(self, *width_expr)?,
                negative: *neg,
            }),
            // A single-index selector on a fixed array is captured as the
            // element select `array[index]`; only its index is the selector.
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) if indices.len() == 1 => {
                Ok(IrStreamSelector::Index(lower_integral(self, indices[0])?))
            }
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    /// Return constant logical indices selected by a `with` expression.  A
    /// `None` result means at least one bound is runtime-valued and must stay
    /// in the typed runtime selector path.
    pub(in super::super) fn static_stream_selector_indices(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<i128>>, String> {
        let range = |left: i128, right: i128| -> Result<Vec<i128>, String> {
            let distance = left.abs_diff(right);
            let count = distance
                .checked_add(1)
                .ok_or_else(|| format!("streaming selector range overflows in `{path}`"))?;
            if count > u128::from(LLG_MAX_WIDTH) {
                return Err(format!(
                    "streaming selector in `{path}` exceeds the runtime maximum width"
                ));
            }
            let count = usize::try_from(count)
                .map_err(|_| format!("streaming selector is too large in `{path}`"))?;
            // A `with` range streams in storage order like a slice (SV
            // 11.4.14.4): ascending here; descending fixed arrays re-orient.
            let low = left.min(right);
            (0..count)
                .map(|offset| {
                    let offset = i128::try_from(offset)
                        .map_err(|_| format!("streaming selector overflows in `{path}`"))?;
                    low.checked_add(offset)
                        .ok_or_else(|| format!("streaming selector overflows in `{path}`"))
                })
                .collect()
        };
        let constant = |this: &Self, bound: NodeId| this.eval_bound_i128(bound).ok();
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(constant(self, *index).map(|index| vec![index]))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                let (Some(left), Some(right)) = (constant(self, *left), constant(self, *right))
                else {
                    return Ok(None);
                };
                range(left, right).map(Some)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let (Some(base), Some(width)) =
                    (constant(self, *base_expr), constant(self, *width_expr))
                else {
                    return Ok(None);
                };
                if width <= 0 {
                    return Err(format!(
                        "streaming indexed selector width must be positive in `{path}`"
                    ));
                }
                let end = if *neg {
                    base.checked_sub(width - 1)
                } else {
                    base.checked_add(width - 1)
                }
                .ok_or_else(|| format!("streaming selector overflows in `{path}`"))?;
                range(base, end).map(Some)
            }
            // A single-index selector on a fixed array is captured as the
            // element select `array[index]`; only its index is the selector.
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) if indices.len() == 1 => {
                Ok(constant(self, indices[0]).map(|index| vec![index]))
            }
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    /// Shape of a one-dimensional fixed array operand of a `with` range that
    /// has no model array storage (a ref or const-ref formal, automatic
    /// local, member, row or call result). Such an operand is lowered as its
    /// whole declaration-order image. `None` means the operand is not a fixed
    /// unpacked array.
    pub(in super::super) fn fixed_image_shape(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<FixedImageShape>, String> {
        let Some(descriptor) = self.query_descriptor(node) else {
            return Ok(None);
        };
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Ok(None);
        };
        let [bounds] = dimensions.as_slice() else {
            return Err(Self::multidimensional_with_error(path));
        };
        let element_width = Self::fixed_descriptor_width(element).ok_or_else(|| {
            format!("streaming `with` operand requires fixed bit-stream elements in `{path}`")
        })?;
        let uninitialized = Self::fixed_element_uninitialized(element);
        // An aggregate element is uniform when its default is all X (every
        // leaf four-state) or all zero (every leaf two-state).
        let (uniform, two_state) = match &uninitialized {
            None => (true, element.two_state),
            Some(default) => {
                let all_x = IrConst::integral_default(element_width, false);
                let limb = |words: &[u64], index: usize| words.get(index).copied().unwrap_or(0);
                if (0..all_x.x.len()).all(|index| limb(&default.x, index) == all_x.x[index])
                    && default.z.iter().all(|word| *word == 0)
                {
                    (true, false)
                } else if default
                    .x
                    .iter()
                    .chain(&default.z)
                    .chain(&default.bits)
                    .all(|word| *word == 0)
                {
                    (true, true)
                } else {
                    (false, element.two_state)
                }
            }
        };
        let count = u64::from(bounds.0.abs_diff(bounds.1)) + 1;
        if count
            .checked_mul(u64::from(element_width))
            .is_none_or(|width| width > u64::from(LLG_MAX_WIDTH))
        {
            return Err(format!(
                "streaming `with` operand without array storage in `{path}` exceeds the runtime maximum width"
            ));
        }
        Ok(Some(FixedImageShape {
            bounds: *bounds,
            element_width,
            two_state,
            uniform,
            fallback: uninitialized
                .unwrap_or_else(|| IrConst::integral_default(element_width, element.two_state)),
        }))
    }

    /// Whether a `with` selector is constant and selects only elements
    /// inside `bounds`. Such a selection unpacks into fixed element parts;
    /// any other selection needs the runtime-checked selector path.
    pub(in super::super) fn static_with_in_bounds(
        &self,
        path: &str,
        with_node: NodeId,
        (left, right): (i32, i32),
    ) -> Result<bool, String> {
        let (low, high) = (left.min(right), left.max(right));
        Ok(self
            .static_stream_selector_indices(path, with_node)?
            .is_some_and(|indices| {
                indices
                    .iter()
                    .all(|index| (i128::from(low)..=i128::from(high)).contains(index))
            }))
    }

    fn fixed_stream_parts(
        &self,
        path: &str,
        array: &ArrayInfo,
        selected: Option<&[i128]>,
    ) -> Result<Vec<IrExpr>, String> {
        if array.dims.is_empty() {
            return Err(format!(
                "fixed streaming array has no dimensions in `{path}`"
            ));
        }
        if array.real {
            return Err(format!(
                "real array streaming operand is not supported in `{path}`"
            ));
        }
        let indices = selected
            .map(|indices| Self::fixed_stream_storage_order(array, indices))
            .unwrap_or_else(|| {
                let (left, right) = array.dims[0];
                let step = if left <= right { 1 } else { -1 };
                let mut values = Vec::new();
                let mut index = i128::from(left);
                loop {
                    values.push(index);
                    if index == i128::from(right) {
                        break;
                    }
                    index += i128::from(step);
                }
                values
            });
        let rest = inside_array_index_vectors(&array.dims[1..]);
        let rest_count = u128::try_from(rest.len())
            .map_err(|_| format!("fixed streaming array is too large in `{path}`"))?;
        let total = u128::try_from(indices.len())
            .ok()
            .and_then(|count| count.checked_mul(rest_count))
            .ok_or_else(|| format!("fixed streaming array is too large in `{path}`"))?;
        let width = total
            .checked_mul(u128::from(array.elem_width))
            .ok_or_else(|| format!("fixed streaming array width overflows in `{path}`"))?;
        if width > u128::from(LLG_MAX_WIDTH) {
            return Err(format!(
                "fixed streaming array in `{path}` exceeds the runtime maximum width"
            ));
        }
        let mut parts = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        for index in indices {
            let prefix = lhs_integer_expr(index);
            for suffix in &rest {
                let mut coordinates = Vec::with_capacity(1 + suffix.len());
                coordinates.push(prefix.clone());
                coordinates.extend(
                    suffix
                        .iter()
                        .map(|value| lhs_integer_expr(i128::from(*value))),
                );
                parts.push(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(array.ir),
                        indices: coordinates,
                        elem_sel: IrElemSel::Whole,
                    },
                    array.elem_width,
                    array.signed,
                    None,
                ));
            }
        }
        Ok(parts)
    }

    fn lower_stream_container_static(
        &self,
        path: &str,
        container: usize,
        indices: &[i128],
    ) -> Result<IrExpr, String> {
        let info = self
            .model
            .containers
            .get(container)
            .ok_or_else(|| format!("streaming container index is out of bounds in `{path}`"))?;
        let (width, signed, _) = info
            .element
            .packed()
            .ok_or_else(|| format!("streaming container requires a packed element in `{path}`"))?;
        let parts = indices
            .iter()
            .map(|index| {
                IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Get {
                        container,
                        index: Box::new(lhs_integer_expr(*index)),
                    })),
                    width,
                    signed,
                    None,
                )
            })
            .collect::<Vec<_>>();
        Self::join_bitstream_parts(path, parts)
    }

    /// Order ascending `with` indices by the fixed array's declaration, which
    /// is its storage order; a descending array streams its high index first.
    pub(in super::super) fn fixed_stream_storage_order(
        array: &ArrayInfo,
        indices: &[i128],
    ) -> Vec<i128> {
        let mut indices = indices.to_vec();
        if array.dims.first().is_some_and(|(left, right)| left > right) {
            indices.reverse();
        }
        indices
    }

    /// SV 11.4.14.4 admits a `with` range only on a one-dimensional unpacked
    /// array; the pinned frontend does not diagnose a multidimensional operand.
    pub(in super::super) fn multidimensional_with_error(path: &str) -> String {
        format!(
            "streaming `with` range requires a one-dimensional unpacked array (SV 11.4.14.4) in `{path}`"
        )
    }

    pub(in super::super) fn lower_stream_operand(
        &mut self,
        path: &str,
        value_node: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(container) = self.container_of(value_node) {
            let element = self
                .model
                .containers
                .get(container.ir)
                .ok_or_else(|| format!("streaming container is out of bounds in `{path}`"))?;
            if !matches!(
                element.kind,
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
            ) {
                return Err(format!(
                    "associative arrays are not legal streaming operands in `{path}`"
                ));
            }
            let (_, _, _) = element.element.packed().ok_or_else(|| {
                format!("streaming container operand must have a packed element in `{path}`")
            })?;
            if let Some(with_node) = with_node {
                if let Some(indices) = self.static_stream_selector_indices(path, with_node)? {
                    return self.lower_stream_container_static(path, container.ir, &indices);
                }
                let selector = self.lower_stream_selector(path, with_node)?;
                return Ok(IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                        container: container.ir,
                        slice: 1,
                        direction: IrStreamDirection::LeftToRight,
                        selector: Some(selector),
                    })),
                    LLG_MAX_WIDTH,
                    false,
                    None,
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                    container: container.ir,
                    slice: 1,
                    direction: IrStreamDirection::LeftToRight,
                    selector: None,
                })),
                LLG_MAX_WIDTH,
                false,
                None,
            ));
        }
        if let Some(array) = self.array_of(value_node).cloned() {
            let selected = match with_node {
                Some(with_node) => self.static_stream_selector_indices(path, with_node)?,
                None => None,
            };
            if let Some(selected) = selected {
                if array.dims.len() != 1 {
                    return Err(Self::multidimensional_with_error(path));
                }
                let parts = self.fixed_stream_parts(path, &array, Some(&selected))?;
                return Self::join_bitstream_parts(path, parts);
            }
            let Some(with_node) = with_node else {
                let parts = self.fixed_stream_parts(path, &array, None)?;
                return Self::join_bitstream_parts(path, parts);
            };
            if array.real {
                return Err(format!(
                    "real array streaming operand is not supported in `{path}`"
                ));
            }
            if array.dims.len() != 1 {
                return Err(format!(
                    "runtime `with` selector on a multidimensional fixed streaming source is not supported in `{path}`"
                ));
            }
            let selector = self.lower_stream_selector(path, with_node)?;
            return Ok(IrExpr::new(
                IrExprKind::FixedStream {
                    array: array.ir,
                    selector: Box::new(selector),
                },
                LLG_MAX_WIDTH,
                false,
                None,
            ));
        }
        if let Some(with_node) = with_node {
            let Some(shape) = self.fixed_image_shape(path, value_node)? else {
                return Err(format!(
                    "streaming `with` selector requires a one-dimensional unpacked array in `{path}`"
                ));
            };
            return self.lower_fixed_image_stream(path, value_node, with_node, shape);
        }
        if let Some(value) = self.lower_bitstream_source(path, value_node)? {
            return Ok(value);
        }
        self.lower_expr(path, value_node)
    }

    /// Lower a `with` selection of an image-represented fixed array. The
    /// whole image is evaluated once; a constant selection keeps its static
    /// width, a runtime one carries its width at runtime.
    fn lower_fixed_image_stream(
        &mut self,
        path: &str,
        value_node: NodeId,
        with_node: NodeId,
        shape: FixedImageShape,
    ) -> Result<IrExpr, String> {
        let image = match self.lower_bitstream_source(path, value_node)? {
            Some(value) => value,
            None => self.lower_expr(path, value_node)?,
        };
        let count = u64::from(shape.bounds.0.abs_diff(shape.bounds.1)) + 1;
        if image.is_real() || u64::from(image.width) != count * u64::from(shape.element_width) {
            return Err(format!(
                "streaming `with` operand has no packed image of its declared shape in `{path}`"
            ));
        }
        let width = match self.static_stream_selector_indices(path, with_node)? {
            Some(indices) => u32::try_from(indices.len())
                .ok()
                .and_then(|count| count.checked_mul(shape.element_width))
                .filter(|width| *width <= LLG_MAX_WIDTH)
                .ok_or_else(|| {
                    format!("streaming selection in `{path}` exceeds the runtime maximum width")
                })?,
            None => LLG_MAX_WIDTH,
        };
        let selector = self.lower_stream_selector(path, with_node)?;
        let fallback_width = shape.element_width;
        Ok(IrExpr::new(
            IrExprKind::FixedImageStream {
                image: Box::new(image),
                bounds: shape.bounds,
                element_width: shape.element_width,
                fallback: Box::new(IrExpr::new(
                    IrExprKind::Const(shape.fallback),
                    fallback_width,
                    false,
                    None,
                )),
                selector: Box::new(selector),
            },
            width,
            false,
            None,
        ))
    }
}
