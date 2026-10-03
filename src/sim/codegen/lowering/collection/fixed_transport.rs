//! Fixed values lower to descriptor operations rather than leaf expansion.
use super::*;
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
                    parts.push(IrFixedValue::Array(self.fixed_with_view(
                        path,
                        stream.value,
                        with_node,
                    )?));
                    continue;
                }
                let part = self.lower_fixed_value(path, stream.value)?;
                if matches!(part, IrFixedValue::Stream { .. }) {
                    // A nested stream has no element-shaped extent of its own.
                    return Err("nested descriptor streams are not supported".into());
                }
                parts.push(part);
            }
            return Ok(IrFixedValue::Stream { parts, slice });
        }
        let view = self.fixed_memory_view(path, node)?;
        if !self.model.arrays[view.array].sparse() {
            return Err("descriptor value requires descriptor storage".into());
        }
        Ok(IrFixedValue::Array(view))
    }
}

impl Codegen<'_> {
    /// A constant in-bounds `with` range of a one-dimensional descriptor
    /// array streams like the equivalent slice (SV 11.4.14.4), so it is a
    /// sliced view in storage order. A runtime or out-of-bounds range would
    /// give the stream a runtime extent, which descriptor streams cannot
    /// represent.
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
    /// Scatter an oversized array value whose first-dimension elements are
    /// array-valued pattern targets. The source is evaluated once into a
    /// lexical snapshot and target selectors are frozen before any write, so
    /// later targets never observe earlier outputs; each target then receives
    /// one descriptor row copy.
    pub(in super::super) fn lower_descriptor_pattern_scatter(
        &mut self,
        path: &str,
        rhs: NodeId,
        targets: &[(NodeId, TypeDescriptor)],
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let Some(source) = self.query_descriptor(rhs).cloned() else {
            return Ok(None);
        };
        let TypeShape::FixedArray { dimensions, .. } = &source.shape else {
            return Ok(None);
        };
        if dimensions.len() < 2
            || Self::fixed_descriptor_width_bits(&source)
                .is_none_or(|width| width <= u64::from(LLG_MAX_WIDTH))
            || !targets
                .iter()
                .all(|(_, descriptor)| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
        {
            return Ok(None);
        }
        let bounds = dimensions[0];
        if u64::try_from(targets.len()).ok() != Some(u64::from(bounds.0.abs_diff(bounds.1)) + 1) {
            return Err(format!(
                "assignment-pattern lvalue in `{path}` does not match its source rows"
            ));
        }
        let mut statements = Vec::new();
        let mut views = Vec::with_capacity(targets.len());
        for (target, _) in targets {
            if nba
                && (self.proc_local_target(*target).is_some()
                    || self.subroutine_auto_target(*target))
            {
                return Err(format!(
                    "nonblocking assignment to an automatic assignment-pattern target in `{path}` is not supported"
                ));
            }
            let mut view = self.fixed_memory_view(path, *target)?;
            if !self.model.arrays[view.array].sparse() {
                return Err(format!(
                    "assignment-pattern lvalue row in `{path}` requires descriptor storage for oversized scatter"
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
            views.push(view);
        }
        let snapshot = self.fixed_activation_array(rhs)?;
        let value = self.lower_fixed_value(path, rhs)?;
        // The snapshot precedes selector capture in evaluation order: the RHS is
        // evaluated before the destination selectors (IEEE 1800-2009 §10.9).
        let mut ordered = vec![
            IrStmt::FixedArrayDeclare(snapshot.ir),
            IrStmt::FixedValueAssign {
                dst: self.fixed_view_at(snapshot.ir, &[]),
                src: Box::new(value),
                nba: false,
            },
        ];
        ordered.append(&mut statements);
        for (offset, view) in views.into_iter().enumerate() {
            let offset =
                i32::try_from(offset).map_err(|_| "assignment-pattern row offset overflows")?;
            let index = if bounds.0 >= bounds.1 {
                bounds.0 - offset
            } else {
                bounds.0 + offset
            };
            let row = self.fixed_view_at(snapshot.ir, &[lhs_integer_expr(i128::from(index))]);
            if row.total != view.total {
                return Err(format!(
                    "assignment-pattern lvalue row shape mismatch in `{path}`"
                ));
            }
            ordered.push(IrStmt::FixedValueAssign {
                dst: view,
                src: Box::new(IrFixedValue::Array(row)),
                nba,
            });
        }
        Ok(Some(IrStmt::Block(ordered)))
    }
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
