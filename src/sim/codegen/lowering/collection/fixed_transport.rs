//! Fixed values lower to descriptor operations rather than leaf expansion.
use super::*;
use crate::sim::codegen::lowering::containers::PatternAssignmentKind;
use crate::sim::ir::IrFixedValue;

impl Codegen<'_> {
    pub(in super::super) fn lower_fixed_value(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrFixedValue, String> {
        let node = match self.descriptor_cast(node) {
            DescriptorCast::Convert(cast, operand) => {
                let value = self.lower_fixed_value(path, operand)?;
                let array = self.fixed_activation_array(cast)?.ir;
                return Ok(IrFixedValue::Convert {
                    value: Box::new(value),
                    array,
                });
            }
            DescriptorCast::Storage(node) => node,
        };
        if let NodeKind::FuncCall { name, callee, .. } = self.kind(node) {
            let (name, callee) = (name.clone(), *callee);
            let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
            if self.nonflatten_function(function) {
                let result = self.fixed_activation_array(function)?;
                let expression = self.lower_func_call_expr(path, node, &name, callee)?;
                let IrExprKind::CallFn(expression) = expression.kind else {
                    return Err("fixed call requires typed operands".into());
                };
                let mut args = expression.args;
                let outputs = self.model.funcs[expression.f]
                    .formals
                    .iter()
                    .filter(|formal| formal.is_address())
                    .count();
                args.insert(outputs - 1, IrCallArg::FixedArray(result.ir));
                return Ok(IrFixedValue::Call {
                    array: result.ir,
                    call: Box::new(IrCall::new(
                        expression.f,
                        args,
                        expression.depth,
                        Vec::new(),
                        Vec::new(),
                    )),
                });
            }
        }
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Conditional,
            operands,
            ..
        }) = self.kind(node)
        {
            let operands = operands.clone();
            let selector = self.lower_boolean_expr(path, operands[0])?;
            let left = self.lower_fixed_value(path, operands[1])?;
            let right = self.lower_fixed_value(path, operands[2])?;
            let descriptor = self
                .query_descriptor(node)
                .ok_or("fixed conditional has no shape")?;
            let element_cells = if let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape
            {
                dimensions
                    .iter()
                    .skip(1)
                    .try_fold(1u64, |total, (left, right)| {
                        total.checked_mul(u64::from(left.abs_diff(*right)) + 1)
                    })
                    .ok_or("fixed conditional element count overflows")?
            } else {
                1
            };
            return Ok(IrFixedValue::Conditional {
                selector: Box::new(selector),
                left: Box::new(left),
                right: Box::new(right),
                element_cells,
            });
        }
        if let NodeKind::Expr(ExprKind::Streaming {
            direction,
            slice_size,
            streams,
        }) = self.kind(node)
        {
            let slice = if *direction == DbStreamingDirection::LeftToRight {
                0
            } else {
                u32::try_from((*slice_size).max(1)).map_err(|_| "fixed stream slice overflows")?
            };
            let streams = streams.clone();
            let mut parts = Vec::new();
            for stream in streams {
                if let Some(with_node) = stream.with_expr {
                    parts.push(self.lower_fixed_with_part(path, stream.value, with_node)?);
                    continue;
                }
                if matches!(
                    self.kind(stream.value),
                    NodeKind::Expr(ExprKind::Streaming { .. })
                ) {
                    match self.lower_fixed_value(path, stream.value)? {
                        // A nested `>>` stream is the concatenation of its
                        // operands, so they join this stream directly.
                        IrFixedValue::Stream {
                            parts: inner,
                            slice: 0,
                        } => parts.extend(inner),
                        // Other nested streams keep their own reordering and
                        // are imaged lazily as one operand.
                        nested => parts.push(nested),
                    }
                    continue;
                }
                parts.push(self.lower_fixed_value(path, stream.value)?);
            }
            return Ok(IrFixedValue::Stream { parts, slice });
        }
        if let Ok(view) = self.fixed_memory_view(path, node) {
            let array = &self.model.arrays[view.array];
            if array.sparse() {
                return Ok(IrFixedValue::Array(view));
            }
            if !array.real && !array.is_net() && array.elem_width != 0 {
                return Ok(IrFixedValue::Dense(view));
            }
        }
        // Dense arrays, small records and integral values fit one packed
        // value; they enter descriptor transport as cells, never per element.
        self.lower_fixed_packed(path, node)
    }

    /// A packed bit-stream operand as descriptor cells: an array keeps its
    /// element as the cell, any other value is one cell.
    fn lower_fixed_packed(&mut self, path: &str, node: NodeId) -> Result<IrFixedValue, String> {
        if self.container_of(node).is_some() {
            return Err(format!(
                "resizable container operand of an oversized fixed stream in `{path}` is not supported"
            ));
        }
        let value = match self.lower_bitstream_source(path, node)? {
            Some(value) => value,
            None => self.lower_expr(path, node)?,
        };
        if value.is_real() || value.width == 0 {
            return Err(format!(
                "descriptor operand in `{path}` requires an integral bit-stream value"
            ));
        }
        let cell_width = self
            .query_descriptor(node)
            .and_then(|descriptor| match &descriptor.shape {
                TypeShape::FixedArray { element, .. } => Self::fixed_descriptor_width(element),
                _ => None,
            })
            .filter(|width| *width != 0 && value.width.is_multiple_of(*width))
            .unwrap_or(value.width);
        Ok(IrFixedValue::Packed {
            value: Box::new(value),
            cell_width,
            runtime_sized: false,
        })
    }

    /// A `with` operand of an oversized stream (SV 11.4.14.4). A constant
    /// in-bounds range of descriptor storage is a sliced view; any other
    /// range of descriptor storage is a runtime selection that reads
    /// out-of-bounds elements as their default. A dense or image array is
    /// selected as one packed value whose cells are its elements.
    fn lower_fixed_with_part(
        &mut self,
        path: &str,
        value: NodeId,
        with_node: NodeId,
    ) -> Result<IrFixedValue, String> {
        if let Some(array) = self
            .array_of(value)
            .cloned()
            .filter(|array| self.model.arrays[array.ir].sparse())
        {
            let [bounds] = array.dims.as_slice() else {
                return Err(Self::multidimensional_with_error(path));
            };
            if self.static_with_in_bounds(path, with_node, *bounds)? {
                return Ok(IrFixedValue::Array(
                    self.fixed_with_view(path, value, with_node)?,
                ));
            }
            return Ok(IrFixedValue::Selected {
                array: array.ir,
                selector: self.lower_stream_selector(path, with_node)?,
            });
        }
        if self.container_of(value).is_some() {
            return Err(format!(
                "resizable container operand of an oversized fixed stream in `{path}` is not supported"
            ));
        }
        let element_width = match self.array_of(value) {
            Some(array) => array.elem_width,
            None => {
                self.fixed_image_shape(path, value)?
                    .ok_or_else(|| {
                        format!(
                            "streaming `with` selector requires a one-dimensional unpacked array in `{path}`"
                        )
                    })?
                    .element_width
            }
        };
        let runtime_sized = self
            .static_stream_selector_indices(path, with_node)?
            .is_none();
        let selected = self.lower_stream_operand(path, value, Some(with_node))?;
        if element_width == 0 || selected.is_real() {
            return Err(format!(
                "streaming `with` operand in `{path}` requires integral elements"
            ));
        }
        Ok(IrFixedValue::Packed {
            value: Box::new(selected),
            cell_width: element_width,
            runtime_sized,
        })
    }
}

impl Codegen<'_> {
    /// A constant in-bounds `with` range of a one-dimensional descriptor
    /// array streams like the equivalent slice (SV 11.4.14.4), so it is a
    /// sliced view in storage order. Other ranges are `IrFixedValue::Selected`.
    fn fixed_with_view(
        &mut self,
        path: &str,
        value: NodeId,
        with_node: NodeId,
    ) -> Result<IrMemoryView, String> {
        let array = self
            .array_of(value)
            .cloned()
            .filter(|array| self.model.arrays[array.ir].sparse())
            .ok_or_else(|| {
                format!("descriptor stream `with` operand requires descriptor array storage in `{path}`")
            })?;
        let [(left, right)] = array.dims.as_slice() else {
            return Err(Self::multidimensional_with_error(path));
        };
        let (left, right) = (*left, *right);
        if !self.static_with_in_bounds(path, with_node, (left, right))? {
            return Err(format!(
                "descriptor stream `with` range must be constant and inside the array bounds in `{path}`"
            ));
        }
        let indices = self
            .static_stream_selector_indices(path, with_node)?
            .unwrap_or_default();
        let (Some(low), Some(high)) = (indices.first(), indices.last()) else {
            return Err(format!("empty descriptor stream `with` range in `{path}`"));
        };
        // Bounds were checked against the i32 declaration above.
        let (low, high) = (*low as i32, *high as i32);
        let (view_left, view_right) = if left >= right {
            (high, low)
        } else {
            (low, high)
        };
        let origin = if left >= right {
            u64::from(left.abs_diff(view_left))
        } else {
            u64::from(view_left.abs_diff(left))
        };
        Ok(IrMemoryView {
            array: array.ir,
            origin,
            selectors: Vec::new(),
            sliced: true,
            dims: vec![(view_left, view_right)],
            strides: vec![1],
            total: u64::from(high.abs_diff(low)) + 1,
        })
    }
}

impl Codegen<'_> {
    /// View of `array` with its leading dimensions selected by `coordinates`
    /// (declared index values), spanning every remaining dimension.
    pub(in super::super) fn fixed_view_at(
        &self,
        array: usize,
        coordinates: &[IrExpr],
    ) -> IrMemoryView {
        let dims = &self.model.arrays[array].dims;
        let mut strides = vec![1u64; dims.len()];
        for dimension in (0..dims.len().saturating_sub(1)).rev() {
            let (left, right) = dims[dimension + 1];
            strides[dimension] = strides[dimension + 1] * (u64::from(left.abs_diff(right)) + 1);
        }
        let selectors = coordinates
            .iter()
            .enumerate()
            .map(|(dimension, value)| crate::sim::ir::IrMemorySelector {
                dimension,
                left: dims[dimension].0,
                right: dims[dimension].1,
                stride: strides[dimension],
                value: value.clone(),
            })
            .collect();
        let selected = coordinates.len();
        let total = if selected == 0 {
            self.model.arrays[array].total
        } else {
            strides[selected - 1]
        };
        IrMemoryView {
            array,
            origin: 0,
            selectors,
            sliced: false,
            dims: dims[selected..].to_vec(),
            strides: strides[selected..].to_vec(),
            total,
        }
    }
}

impl Codegen<'_> {
    /// Scatter an array value into the rows of a positional pattern lvalue
    /// through a lexical snapshot (IEEE 1800-2009 10.10). The scatter is used
    /// when the source is oversized or descriptor-stored, or when a row target
    /// lives in descriptor storage, so neither side is ever flattened. The
    /// source is evaluated once and target selectors are frozen before any
    /// write, so later targets never observe earlier outputs. Each row target,
    /// at any nesting depth, receives one row copy from the snapshot cells it
    /// covers in declaration order; a packed leaf receives one snapshot cell.
    pub(in super::super) fn lower_descriptor_pattern_scatter(
        &mut self,
        path: &str,
        rhs: NodeId,
        targets: &[(NodeId, TypeDescriptor)],
        kind: PatternAssignmentKind,
    ) -> Result<Option<IrStmt>, String> {
        let nba = kind == PatternAssignmentKind::Nonblocking;
        let Some(source) = self.query_descriptor(rhs).cloned() else {
            return Ok(None);
        };
        let TypeShape::FixedArray { dimensions, .. } = &source.shape else {
            return Ok(None);
        };
        let row_target =
            |descriptor: &TypeDescriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. });
        if !targets.iter().any(|(_, descriptor)| row_target(descriptor)) {
            return Ok(None);
        }
        // A descriptor-stored source scatters rows even when its total width
        // would fit one packed value: it is never flattened. A small dense
        // source keeps the packed path unless a row target is descriptor
        // storage, whose packed lvalue would enumerate every cell.
        let oversized = Self::fixed_descriptor_width_bits(&source)
            .is_none_or(|width| width > u64::from(LLG_MAX_WIDTH))
            || self.descriptor_operand(rhs);
        let descriptor_row = targets
            .iter()
            .any(|(target, descriptor)| row_target(descriptor) && self.descriptor_operand(*target));
        if !oversized && !descriptor_row {
            return Ok(None);
        }
        let dimensions = dimensions.clone();
        let extents = dimensions
            .iter()
            .map(|(left, right)| u64::from(left.abs_diff(*right)) + 1)
            .collect::<Vec<_>>();
        // Cells covered by one coordinate of each source dimension.
        let mut strides = vec![1u64; extents.len()];
        for dimension in (0..extents.len().saturating_sub(1)).rev() {
            strides[dimension] = strides[dimension + 1]
                .checked_mul(extents[dimension + 1])
                .ok_or("assignment-pattern source is too large")?;
        }
        let total = strides
            .first()
            .zip(extents.first())
            .and_then(|(stride, extent)| stride.checked_mul(*extent))
            .ok_or("assignment-pattern source is too large")?;
        let mut statements = Vec::new();
        let mut placements = Vec::with_capacity(targets.len());
        let mut cell = 0u64;
        let mut sequence = 0;
        let tag = self.new_fn_name(path, "pattern_targets");
        for (target, descriptor) in targets {
            if nba
                && (self.proc_local_target(*target).is_some()
                    || self.subroutine_auto_target(*target))
            {
                return Err(format!(
                    "nonblocking assignment to an automatic assignment-pattern target in `{path}` is not supported"
                ));
            }
            let rank = match &descriptor.shape {
                TypeShape::FixedArray { dimensions, .. } => dimensions.len(),
                _ => 0,
            };
            let prefix = dimensions.len().checked_sub(rank).ok_or_else(|| {
                format!("assignment-pattern lvalue row shape mismatch in `{path}`")
            })?;
            let span = if prefix == 0 {
                total
            } else {
                strides[prefix - 1]
            };
            if !cell.is_multiple_of(span) || cell >= total {
                return Err(format!(
                    "assignment-pattern lvalue in `{path}` does not match its source rows"
                ));
            }
            let coordinates = (0..prefix)
                .map(|dimension| {
                    let ordinal =
                        i64::try_from((cell / strides[dimension]) % extents[dimension])
                            .map_err(|_| "assignment-pattern row offset overflows".to_owned())?;
                    let (left, right) = dimensions[dimension];
                    let index = if left >= right {
                        i64::from(left) - ordinal
                    } else {
                        i64::from(left) + ordinal
                    };
                    i32::try_from(index)
                        .map_err(|_| "assignment-pattern row offset overflows".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            cell += span;
            // A packed leaf of a nested pattern takes one snapshot cell. A net
            // row (net arrays are never descriptor storage) takes the packed
            // image of its snapshot row, so continuous drivers keep their
            // per-leaf net mapping.
            let net_row = rank > 0
                && self
                    .p30_array_prefix_base(*target)
                    .is_some_and(|(array, _)| array.is_net);
            if rank == 0 || net_row {
                let lhs = self.lower_lhs(path, *target)?;
                if nba && matches!(lhs, IrLhs::Ref { .. }) {
                    return Err(format!(
                        "nonblocking assignment through a reference formal in `{path}` is not supported"
                    ));
                }
                let lhs = if kind == PatternAssignmentKind::Continuous {
                    lhs
                } else {
                    let mut captures = Vec::new();
                    let (lhs, _) = self.freeze_call_lhs(lhs, &tag, &mut sequence, &mut captures)?;
                    statements.extend(captures.into_iter().map(
                        |(name, width, signed, two_state, expr)| IrStmt::DeclLocal {
                            name,
                            width,
                            signed,
                            two_state,
                            init: Some(Box::new(expr)),
                        },
                    ));
                    lhs
                };
                placements.push((PatternPlacement::Packed(lhs), coordinates));
                continue;
            }
            let mut view = self.fixed_memory_view(path, *target)?;
            let storage = &self.model.arrays[view.array];
            // A small dense row receives its cells by a copy loop.
            if !storage.sparse() && (storage.real || storage.is_net() || storage.elem_width == 0) {
                return Err(format!(
                    "assignment-pattern lvalue row in `{path}` requires descriptor or dense integral storage for oversized scatter"
                ));
            }
            for selector in &mut view.selectors {
                let name = self.new_fn_name(path, "pattern_selector");
                let (width, signed) = (selector.value.width, selector.value.signed);
                let value = std::mem::replace(
                    &mut selector.value,
                    IrExpr::new(IrExprKind::LocalRead(name.clone()), width, signed, None),
                );
                statements.push(IrStmt::DeclLocal {
                    name,
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                });
            }
            placements.push((PatternPlacement::Row(view), coordinates));
        }
        if cell != total {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` does not match its source rows"
            ));
        }
        let snapshot = self.fixed_activation_array(rhs)?;
        // The snapshot precedes selector capture in evaluation order: the RHS is
        // evaluated before the destination selectors (IEEE 1800-2009 §10.9).
        let mut ordered = vec![IrStmt::FixedArrayDeclare(snapshot.ir)];
        if let Some(construct) =
            self.lower_descriptor_pattern_into(path, rhs, &source, &snapshot)?
        {
            // A pattern source is built in place from once-captured items.
            ordered.extend(construct);
        } else {
            let value = self.lower_fixed_value(path, rhs)?;
            ordered.push(IrStmt::FixedValueAssign {
                dst: self.fixed_view_at(snapshot.ir, &[]),
                src: Box::new(value),
                nba: false,
            });
        }
        ordered.append(&mut statements);
        for (placement, coordinates) in placements {
            let indices = coordinates
                .iter()
                .map(|index| lhs_integer_expr(i128::from(*index)))
                .collect::<Vec<_>>();
            match placement {
                PatternPlacement::Packed(lhs) => {
                    let value = snapshot_row_image(&snapshot, &dimensions, &coordinates)?;
                    if packed_lhs_width(&self.model, &lhs) != Some(value.width) {
                        return Err(format!(
                            "assignment-pattern lvalue target in `{path}` does not match its source row width"
                        ));
                    }
                    let rhs = apply_lhs_assignment_context(&self.model, &lhs, value);
                    ordered.push(IrStmt::Assign { lhs, rhs, nba });
                }
                PatternPlacement::Row(view) => {
                    let row = self.fixed_view_at(snapshot.ir, &indices);
                    if row.total != view.total {
                        return Err(format!(
                            "assignment-pattern lvalue row shape mismatch in `{path}`"
                        ));
                    }
                    if !self.model.arrays[view.array].sparse() {
                        ordered.extend(self.dense_row_scatter(
                            path,
                            &view,
                            snapshot.ir,
                            &coordinates,
                            nba,
                        )?);
                        continue;
                    }
                    ordered.push(IrStmt::FixedValueAssign {
                        dst: view,
                        src: Box::new(IrFixedValue::Array(row)),
                        nba,
                    });
                }
            }
        }
        Ok(Some(IrStmt::Block(ordered)))
    }
}

/// The packed image of the snapshot cells under a coordinate prefix, in
/// declaration order (leftmost cell in the most significant bits).
fn snapshot_row_image(
    snapshot: &ArrayInfo,
    dimensions: &[(i32, i32)],
    prefix: &[i32],
) -> Result<IrExpr, String> {
    let rest = dimensions
        .get(prefix.len()..)
        .ok_or("assignment-pattern row prefix exceeds its source")?;
    let count = rest.iter().try_fold(1u64, |count, (left, right)| {
        count.checked_mul(u64::from(left.abs_diff(*right)) + 1)
    });
    let width = count
        .and_then(|count| count.checked_mul(u64::from(snapshot.elem_width)))
        .and_then(|width| u32::try_from(width).ok())
        .filter(|width| *width <= LLG_MAX_WIDTH)
        .ok_or("assignment-pattern packed row exceeds the packed value limit")?;
    let mut parts = Vec::new();
    let mut coordinates = prefix
        .iter()
        .copied()
        .chain(rest.iter().map(|(left, _)| *left))
        .collect::<Vec<_>>();
    loop {
        parts.push(IrExpr::new(
            IrExprKind::ArrayRead {
                arr: snapshot.ir,
                indices: coordinates
                    .iter()
                    .map(|index| lhs_integer_expr(i128::from(*index)))
                    .collect(),
                elem_sel: IrElemSel::Whole,
            },
            snapshot.elem_width,
            snapshot.signed,
            None,
        ));
        // Advance the trailing coordinates as an odometer in declaration order.
        let mut dimension = dimensions.len();
        loop {
            if dimension == prefix.len() {
                return Ok(if parts.len() == 1 {
                    parts.swap_remove(0)
                } else {
                    IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
                });
            }
            dimension -= 1;
            let (left, right) = dimensions[dimension];
            if coordinates[dimension] != right {
                coordinates[dimension] += if left >= right { -1 } else { 1 };
                break;
            }
            coordinates[dimension] = left;
        }
    }
}

/// Where one positional pattern target receives its snapshot cells.
enum PatternPlacement {
    /// A packed leaf or net row written from the packed image of its cells.
    Packed(IrLhs),
    /// An unpacked row view written from the snapshot row at its prefix.
    Row(IrMemoryView),
}

impl Codegen<'_> {
    /// Whether a descriptor destination needs general value transport. Whole
    /// copies, aligned single-array streams and blocking whole-result calls
    /// keep their direct RTL-002 operations; patterns use descriptor patterns.
    pub(in super::super) fn descriptor_value_transport(
        &mut self,
        path: &str,
        dst: &IrMemoryView,
        rhs: NodeId,
        blocking: bool,
    ) -> Result<bool, String> {
        if self.converting_descriptor_cast(rhs) {
            return Ok(self.descriptor_operand(rhs));
        }
        let source = self.p30_unwrap_cast(rhs);
        if self.assignment_pattern_operands(path, source)?.is_some() {
            return Ok(false);
        }
        let selected = !dst.selectors.is_empty()
            || dst.sliced
            || dst.total != self.model.arrays[dst.array].total;
        Ok(match self.kind(source) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                ..
            }) => true,
            NodeKind::Expr(ExprKind::Streaming {
                streams,
                slice_size,
                ..
            }) => {
                let slice = u32::try_from((*slice_size).max(1)).unwrap_or(u32::MAX);
                let aligned = matches!(streams.as_slice(), [stream]
                if stream.with_expr.is_none()
                    && self.array_of(stream.value).is_some_and(|source| {
                        source.elem_width.is_multiple_of(slice)
                            || slice.is_multiple_of(source.elem_width)
                    }));
                selected || !aligned
            }
            NodeKind::FuncCall { .. } => selected || !blocking,
            _ => selected || self.array_of(source).is_none(),
        })
    }
}

impl Codegen<'_> {
    pub(in super::super) fn fixed_memory_view(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrMemoryView, String> {
        let function = self.func.clone();
        let depth = self.depth_arg.clone();
        let instance = self.inst;
        EmitCtx::new(
            self,
            path.to_owned(),
            instance,
            &depth,
            function,
            None,
            false,
        )
        .lower_memory_view("fixed value", node)
    }
}

/// How an operand reaches descriptor storage through explicit casts.
enum DescriptorCast {
    /// Casts that keep the cell layout and state domain are transparent.
    Storage(NodeId),
    /// The first cast that reshapes the bit stream or narrows its element
    /// domain to two-state, with that cast's operand.
    Convert(NodeId, NodeId),
}

impl Codegen<'_> {
    fn descriptor_cast(&self, mut node: NodeId) -> DescriptorCast {
        while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(node) {
            let operand = *operand;
            if self.cast_changes_descriptor(node, operand) {
                return DescriptorCast::Convert(node, operand);
            }
            node = operand;
        }
        DescriptorCast::Storage(node)
    }

    /// A fixed-array to fixed-array cast changes descriptor cells when the
    /// extents or cell width differ, or when a four-state source enters a
    /// two-state element domain (SV 6.24.3); signedness is irrelevant to the
    /// stored bit stream.
    fn cast_changes_descriptor(&self, cast: NodeId, operand: NodeId) -> bool {
        let (Some(target), Some(source)) =
            (self.query_descriptor(cast), self.query_descriptor(operand))
        else {
            return false;
        };
        let (
            TypeShape::FixedArray {
                dimensions: target_dimensions,
                element: target_element,
            },
            TypeShape::FixedArray {
                dimensions: source_dimensions,
                element: source_element,
            },
        ) = (&target.shape, &source.shape)
        else {
            return false;
        };
        let extents = |dimensions: &[(i32, i32)]| {
            dimensions
                .iter()
                .map(|(left, right)| left.abs_diff(*right))
                .collect::<Vec<_>>()
        };
        let width = Self::fixed_descriptor_width(target_element);
        width.is_some()
            && (extents(target_dimensions) != extents(source_dimensions)
                || width != Self::fixed_descriptor_width(source_element)
                || (target_element.two_state && !source_element.two_state))
    }

    pub(in super::super) fn converting_descriptor_cast(&self, node: NodeId) -> bool {
        matches!(self.descriptor_cast(node), DescriptorCast::Convert(..))
    }

    /// Whether `node` lowers to an [`IrFixedValue`] over descriptor storage:
    /// selected descriptor views, descriptor-shaped casts and conditionals of
    /// such operands, and descriptor-returning calls.
    pub(in super::super) fn descriptor_operand(&self, node: NodeId) -> bool {
        let node = match self.descriptor_cast(node) {
            DescriptorCast::Convert(_, operand) => return self.descriptor_operand(operand),
            DescriptorCast::Storage(node) => node,
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                operands,
                ..
            }) => {
                operands.len() == 3
                    && self.query_descriptor(node).is_some_and(|descriptor| {
                        matches!(&descriptor.shape, TypeShape::FixedArray { element, .. }
                            if Self::fixed_descriptor_width(element).is_some())
                    })
                    && self.descriptor_operand(operands[1])
                    && self.descriptor_operand(operands[2])
            }
            NodeKind::FuncCall { callee, .. } => {
                callee.is_some_and(|function| self.nonflatten_function(function))
            }
            _ => self
                .p30_array_prefix_base(node)
                .is_some_and(|(array, _)| self.model.arrays[array.ir].sparse()),
        }
    }
}
