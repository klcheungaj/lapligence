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
        // leaf four-state) or all zero (every leaf two-state). Otherwise its
        // two-state leaves are the bits whose default is a known zero.
        let (two_state, two_state_runs) = match &uninitialized {
            None => (element.two_state, Vec::new()),
            Some(default) => {
                let limb = |words: &[u64], index: u32| {
                    words.get((index / 64) as usize).copied().unwrap_or(0) >> (index % 64) & 1
                };
                let known = |bit: u32| limb(&default.x, bit) == 0 && limb(&default.z, bit) == 0;
                let mut runs = Vec::new();
                let mut bit = 0;
                while bit < element_width {
                    if !known(bit) {
                        bit += 1;
                        continue;
                    }
                    let lsb = bit;
                    while bit < element_width && known(bit) {
                        bit += 1;
                    }
                    runs.push((lsb, bit - lsb));
                }
                match runs.as_slice() {
                    [] => (false, Vec::new()),
                    [(0, width)] if *width == element_width => (true, Vec::new()),
                    _ => (element.two_state, runs),
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
            two_state_runs,
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

    /// Whether a streaming operand or bit-stream cast source has a size known
    /// only at run time and so streams through a runtime-sized bit stream: a
    /// string, a resizable container, or a streaming concatenation with such
    /// an operand.
    pub(in super::super) fn is_runtime_stream_source(&self, path: &str, node: NodeId) -> bool {
        if self.container_of(node).is_some()
            || self.is_string_expr(path, node)
            || self.native_record_stream_leaves(node).is_some()
        {
            return true;
        }
        if let NodeKind::Expr(ExprKind::Streaming { streams, .. }) = self.kind(node) {
            return streams
                .iter()
                .any(|stream| self.is_runtime_stream_source(path, stream.value));
        }
        self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(
                descriptor.shape,
                TypeShape::String | TypeShape::Container { .. }
            )
        })
    }

    /// Lower a streaming concatenation into a runtime-sized bit stream whose
    /// length is not bounded by the packed value width. `None` means `node`
    /// is not a streaming concatenation.
    pub(in super::super) fn lower_bit_stream(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrBitStream>, String> {
        let NodeKind::Expr(ExprKind::Streaming {
            direction,
            slice_size,
            streams,
        }) = self.kind(node)
        else {
            return Ok(None);
        };
        let (direction, slice_size, streams) = (*direction, *slice_size, streams.clone());
        if streams.is_empty() {
            return Err(format!("empty streaming concatenation in `{path}`"));
        }
        let mut segments = Vec::with_capacity(streams.len());
        for stream in streams {
            segments.push(self.lower_stream_segment(path, stream.value, stream.with_expr)?);
        }
        Ok(Some(IrBitStream {
            segments,
            slice: Self::stream_slice(path, slice_size)?,
            direction: match direction {
                DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
            },
            unpack: None,
        }))
    }

    /// A bit stream of one bit-stream cast source (SV 6.24.3).
    pub(in super::super) fn lower_bit_stream_source(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrBitStream, String> {
        if let Some(stream) = self.lower_bit_stream(path, node)? {
            return Ok(stream);
        }
        Ok(IrBitStream {
            segments: vec![self.lower_stream_segment(path, node, None)?],
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
            unpack: None,
        })
    }

    pub(in super::super) fn stream_slice(path: &str, slice_size: u64) -> Result<u32, String> {
        if slice_size == 0 {
            return Ok(1);
        }
        u32::try_from(slice_size)
            .map_err(|_| format!("streaming slice size is too large in `{path}`"))
    }

    /// Leaves of a module or static unpacked struct that has string or
    /// resizable container members, in declaration order (SV 11.4.14.1
    /// streams struct members in turn). `None` for every other operand.
    fn native_record_stream_leaves(&self, node: NodeId) -> Option<Vec<AggregateMemberInfo>> {
        let (_, aggregate) = self.unpacked_aggregate_info(node)?;
        if aggregate.kind != AggregateKind::UnpackedStruct
            || !aggregate
                .leaves
                .iter()
                .any(|leaf| leaf.object.is_some() || leaf.container.is_some())
        {
            return None;
        }
        Some(aggregate.leaves)
    }

    /// The members of a struct with string or resizable members as one
    /// nested stream of its leaves.
    fn native_record_stream(
        &mut self,
        path: &str,
        leaves: Vec<AggregateMemberInfo>,
    ) -> Result<IrStreamSegment, String> {
        let mut segments = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            let segment = if let Some(object) = leaf.object {
                if self.model.objects.get(object).map(|object| object.ty)
                    != Some(IrObjectType::String)
                {
                    return Err(format!(
                        "streaming struct member `{}` is not a bit-stream type in `{path}`",
                        aggregate_path_suffix(&leaf.path)
                    ));
                }
                IrStreamSegment::String(IrStringExpr::Read(object))
            } else if let Some(container) = &leaf.container {
                let info = self.model.containers.get(container.ir).ok_or_else(|| {
                    format!("streaming struct member container is out of bounds in `{path}`")
                })?;
                if !info.element.is_packed() {
                    return Err(format!(
                        "streaming struct member `{}` with nested container, string or record elements is not supported in `{path}`",
                        aggregate_path_suffix(&leaf.path)
                    ));
                }
                IrStreamSegment::Container {
                    container: container.ir,
                    selector: None,
                }
            } else {
                let value = self.aggregate_leaf_read(&leaf)?;
                if value.is_real() {
                    return Err(format!(
                        "streaming struct member `{}` is real in `{path}`",
                        aggregate_path_suffix(&leaf.path)
                    ));
                }
                IrStreamSegment::Packed(value)
            };
            segments.push(segment);
        }
        Ok(IrStreamSegment::Nested(Box::new(IrBitStream {
            segments,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
            unpack: None,
        })))
    }

    /// One operand of a runtime-sized bit stream, in stream order.
    fn lower_stream_segment(
        &mut self,
        path: &str,
        value: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<IrStreamSegment, String> {
        if with_node.is_none() {
            if let Some(stream) = self.lower_bit_stream(path, value)? {
                return Ok(IrStreamSegment::Nested(Box::new(stream)));
            }
        }
        if let Some(container) = self.container_of(value) {
            let info = self
                .model
                .containers
                .get(container.ir)
                .ok_or_else(|| format!("streaming container is out of bounds in `{path}`"))?;
            if !info.element.is_packed() {
                return Err(format!(
                    "streaming operand with nested container, string or record elements is not supported in `{path}`"
                ));
            }
            let associative = matches!(info.kind, IrContainerKind::Associative { .. });
            if associative && with_node.is_some() {
                return Err(format!(
                    "streaming `with` selector requires a one-dimensional unpacked array in `{path}`"
                ));
            }
            let selector = with_node
                .map(|node| self.lower_stream_selector(path, node))
                .transpose()?;
            return Ok(IrStreamSegment::Container {
                container: container.ir,
                selector,
            });
        }
        if with_node.is_none() {
            if let Some(leaves) = self.native_record_stream_leaves(value) {
                return self.native_record_stream(path, leaves);
            }
        }
        if self.is_string_expr(path, value)
            || self
                .query_descriptor(value)
                .is_some_and(|descriptor| descriptor.shape == TypeShape::String)
        {
            if with_node.is_some() {
                return Err(format!(
                    "streaming `with` selector requires a one-dimensional unpacked array in `{path}`"
                ));
            }
            return Ok(IrStreamSegment::String(self.lower_string(path, value)?));
        }
        if self.query_descriptor(value).is_some_and(
            |descriptor| matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Class"),
        ) {
            return Err(format!(
                "streaming a class object's members is not supported in `{path}`"
            ));
        }
        if self
            .query_descriptor(value)
            .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::Container { .. }))
        {
            return Err(format!(
                "streaming operand that is a resizable container without its own storage (a member, element or call result) is not supported in `{path}`"
            ));
        }
        let value = self.lower_stream_operand(path, value, with_node)?;
        if value.is_real() {
            return Err(format!(
                "streaming concatenation of real value in `{path}` is not supported"
            ));
        }
        Ok(IrStreamSegment::Packed(value))
    }

    /// Bit-stream cast of a dynamically sized source (a resizable container,
    /// or a string cast to an unpacked type) to a fixed-size type (`cast`):
    /// the sizes must match, which is known only at run time (SV 6.24.3).
    /// `None` leaves every other cast to the caller.
    pub(in super::super) fn lower_dynamic_bitstream_cast(
        &mut self,
        path: &str,
        cast: NodeId,
        operand: NodeId,
        ty: &TypeInfo,
        two_state: bool,
    ) -> Result<Option<IrExpr>, String> {
        if matches!(
            self.kind(operand),
            NodeKind::Expr(ExprKind::Streaming { .. })
        ) {
            return Ok(None);
        }
        let unpacked_target = self.query_descriptor(cast).is_some_and(|descriptor| {
            matches!(
                descriptor.shape,
                TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
            )
        });
        let container = self.container_of(operand).is_some()
            || self.native_record_stream_leaves(operand).is_some()
            || self
                .query_descriptor(operand)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::Container { .. }));
        let string = self.is_string_expr(path, operand)
            || self
                .query_descriptor(operand)
                .is_some_and(|descriptor| descriptor.shape == TypeShape::String);
        // A string cast to an integral type is a string conversion, not a
        // bit-stream cast (SV 6.16).
        if !container && !(string && unpacked_target) {
            return Ok(None);
        }
        let width = self
            .fixed_value_width(cast)
            .or(ty.width)
            .filter(|width| *width > 0 && *width < LLG_MAX_WIDTH)
            .ok_or_else(|| {
                format!("bit-stream cast target has no supported fixed size in `{path}`")
            })?;
        let stream = self.lower_bit_stream_source(path, operand)?;
        let value = IrExpr::new(
            IrExprKind::StreamToFixed {
                a: Box::new(Self::bit_stream_value(stream)),
                exact: true,
            },
            width,
            !unpacked_target && ty.signed,
            None,
        );
        let value = if two_state || is_two_state_kind(&ty.kind) {
            IrExpr::to_two_state(value)
        } else {
            value
        };
        self.convert_fixed_payload(cast, value).map(Some)
    }

    /// A runtime-sized bit stream as one packed value of runtime width.
    fn bit_stream_value(stream: IrBitStream) -> IrExpr {
        IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::BitStream(Box::new(stream)))),
            LLG_MAX_WIDTH,
            false,
            None,
        )
    }

    pub(in super::super) fn lower_stream_operand(
        &mut self,
        path: &str,
        value_node: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        // Strings and associative arrays have no packed stream form of their
        // own; they stream through a runtime-sized bit stream.
        let associative = self.container_of(value_node).is_some_and(|container| {
            self.model
                .containers
                .get(container.ir)
                .is_some_and(|info| matches!(info.kind, IrContainerKind::Associative { .. }))
        });
        let string = self.container_of(value_node).is_none()
            && (self.is_string_expr(path, value_node)
                || self
                    .query_descriptor(value_node)
                    .is_some_and(|descriptor| descriptor.shape == TypeShape::String));
        if self.query_descriptor(value_node).is_some_and(
            |descriptor| matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Class"),
        ) {
            return Err(format!(
                "streaming a class object's members is not supported in `{path}`"
            ));
        }
        let record = with_node.is_none() && self.native_record_stream_leaves(value_node).is_some();
        if associative || string || record {
            let segment = self.lower_stream_segment(path, value_node, with_node)?;
            return Ok(Self::bit_stream_value(IrBitStream {
                segments: vec![segment],
                slice: 1,
                direction: IrStreamDirection::LeftToRight,
                unpack: None,
            }));
        }
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
