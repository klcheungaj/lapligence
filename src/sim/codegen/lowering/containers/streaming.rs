//! Streaming.

use super::*;

impl<'a> Codegen<'a> {
    /// Lower a fixed-width bit-stream cast into a packed-element dynamic
    /// array or queue.  The target size is derived from the source width, so
    /// the existing value-assignment runtime can replace the destination in a
    /// single operation and apply its declared two-state conversion.
    pub(super) fn lower_bitstream_cast_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        dst: &ContainerInfo,
    ) -> Result<Option<IrStmt>, String> {
        let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(rhs) else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(rhs) else {
            return Ok(None);
        };
        if !matches!(descriptor.shape, TypeShape::Container { .. }) {
            return Ok(None);
        }
        let target = self.model.containers.get(dst.ir).cloned().ok_or_else(|| {
            format!("bit-stream cast target container is out of bounds in `{path}`")
        })?;
        if !matches!(
            target.kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "associative arrays are not legal bit-stream cast destinations in `{path}`"
            ));
        }
        let (element_width, _, _) = target.element.packed().ok_or_else(|| {
            format!("bit-stream cast destination requires a packed container element in `{path}`")
        })?;
        if element_width == 0 {
            return Err(format!(
                "bit-stream cast destination has an empty packed element in `{path}`"
            ));
        }
        let source_descriptor = self.query_descriptor(*operand);
        if source_descriptor.is_some_and(|descriptor| {
            matches!(
                descriptor.shape,
                TypeShape::Container { .. } | TypeShape::String | TypeShape::Opaque { .. }
            )
        }) {
            return Err(format!(
                "dynamic-size, string, or opaque bit-stream source is not supported in `{path}`"
            ));
        }
        let source = match self.lower_bitstream_source(path, *operand)? {
            Some(value) => value,
            None => self.lower_expr(path, *operand)?,
        };
        if source.is_real() {
            return Err(format!(
                "real bit-stream source cannot initialize a resizable container in `{path}`"
            ));
        }
        let count = if source.width == 0 {
            return Err(format!("bit-stream source has no packed width in `{path}`"));
        } else {
            let remainder = source.width % element_width;
            if remainder != 0 {
                return Err(format!(
                    "bit-stream cast source is {} bits but container element width is {} in `{path}`",
                    source.width, element_width
                ));
            }
            usize::try_from(source.width / element_width).map_err(|_| {
                format!("bit-stream cast container element count overflows in `{path}`")
            })?
        };
        if let IrContainerKind::Queue {
            maximum_elements: Some(limit),
        } = target.kind
        {
            if u64::try_from(count).ok().is_none_or(|count| count > limit) {
                return Err(format!(
                    "bit-stream cast result has {count} elements but bounded queue capacity is {limit} in `{path}`"
                ));
            }
        }
        let source_name = format!("_bitstream_container_{}_{}", lhs.0, rhs.0);
        let source_width = source.width;
        let source_signed = source.signed;
        let captured = IrExpr::new(
            IrExprKind::LocalRead(source_name.clone()),
            source_width,
            source_signed,
            None,
        );
        let mut statements = vec![IrStmt::DeclLocal {
            name: source_name,
            width: source_width,
            signed: source_signed,
            init: Some(Box::new(source)),
            two_state: false,
        }];
        let mut values = Vec::with_capacity(count);
        let mut cursor = source_width;
        for _ in 0..count {
            let right = cursor
                .checked_sub(element_width)
                .ok_or_else(|| format!("bit-stream cast source cursor underflow in `{path}`"))?;
            let left = cursor - 1;
            values.push(IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(captured.clone()),
                    left: i64::from(left),
                    right: i64::from(right),
                },
                element_width,
                false,
                None,
            ));
            cursor = right;
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::AssignValues {
            container: dst.ir,
            values,
        })));
        Ok(Some(IrStmt::Block(statements)))
    }

    fn stream_target_contains_container(&self, node: NodeId) -> bool {
        if self.container_of(node).is_some() {
            return true;
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Streaming { streams, .. }) => streams
                .iter()
                .any(|stream| self.stream_target_contains_container(stream.value)),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.stream_target_contains_container(*operand)),
            _ => false,
        }
    }

    /// Declared bounds of a one-dimensional fixed `with` target, from model
    /// array storage or the operand's image shape.
    fn fixed_with_bounds(&self, path: &str, value: NodeId) -> Result<Option<(i32, i32)>, String> {
        if let Some(array) = self.array_of(value) {
            return Ok(match array.dims.as_slice() {
                [bounds] => Some(*bounds),
                _ => None,
            });
        }
        if self.container_of(value).is_some() {
            return Ok(None);
        }
        Ok(self
            .fixed_image_shape(path, value)?
            .map(|shape| shape.bounds))
    }

    /// Lower a streaming assignment whose target contains a packed-element
    /// resizable container or a fixed array selected by a `with` range that
    /// is runtime-valued or partly outside its bounds. The source is kept as
    /// one packed value and the emitter consumes its materialized stream in
    /// target order, so all source reads happen before any writes. A
    /// nonblocking form (fixed targets only) evaluates every selector at
    /// issue and queues each element write.
    pub(super) fn lower_stream_mixed_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let mode = if blocking {
            StreamTargetMode::Blocking
        } else {
            StreamTargetMode::Nonblocking
        };
        let Some(plan) = self.stream_mixed_targets(path, lhs, mode, op)? else {
            return Ok(None);
        };
        let source = self.lower_stream_operand(path, rhs, None)?;
        if source.is_real() {
            return Err(format!(
                "real source is not legal for a streaming assignment in `{path}`"
            ));
        }
        Ok(Some(plan.assign(source, !blocking)))
    }

    /// The checked unpack plan of a streaming target that needs the
    /// `StreamAssign` statement: one with a packed-element resizable container
    /// or a fixed `with` range that is runtime-valued or partly outside its
    /// bounds. `None` means every component is a static packed lvalue.
    pub(in super::super) fn stream_mixed_targets(
        &mut self,
        path: &str,
        lhs: NodeId,
        mode: StreamTargetMode,
        op: Operation,
    ) -> Result<Option<StreamTargetPlan>, String> {
        let blocking = mode == StreamTargetMode::Blocking;
        let NodeKind::Expr(ExprKind::Streaming {
            direction,
            slice_size,
            streams,
        }) = self.kind(lhs)
        else {
            return Ok(None);
        };
        let streams = streams.clone();
        let (direction, slice_size) = (*direction, *slice_size);
        let mut needs_mixed = false;
        let mut has_container = false;
        for stream in &streams {
            if self.stream_target_contains_container(stream.value) {
                needs_mixed = true;
                has_container = true;
                break;
            }
            if let Some(with_node) = stream.with_expr {
                if let Some(bounds) = self.fixed_with_bounds(path, stream.value)? {
                    if !self.static_with_in_bounds(path, with_node, bounds)? {
                        needs_mixed = true;
                    }
                }
            }
        }
        if !needs_mixed {
            return Ok(None);
        }
        if mode == StreamTargetMode::CopyOut && has_container {
            // Resizable copy-out targets keep their existing lvalue path (SIM-020).
            return Ok(None);
        }
        if !blocking && has_container {
            return Err(format!(
                "nonblocking assignment to a streaming container target in `{path}` is not supported"
            ));
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to a streaming container target in `{path}` is not supported"
            ));
        }

        let reversed_fixed = direction == DbStreamingDirection::RightToLeft && !has_container;
        let mut targets = Vec::new();
        // Lvalue nodes of the targets already unpacked, for the selector
        // dependence check below.
        let mut earlier = Vec::new();
        let mut container_count = 0usize;
        for stream in streams {
            // Slang normally stores streaming operands directly in `streams`,
            // but an explicit nested concatenation is still legal. Flatten it
            // while retaining the language order used by normal assignment.
            let mut pending = vec![stream.value];
            while let Some(value) = pending.pop() {
                let concat = match self.kind(value) {
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::Concat,
                        reordered,
                        operands,
                        ..
                    }) => Some((*reordered, operands.clone())),
                    _ => None,
                };
                if let Some((reordered, mut operands)) = concat {
                    if stream.with_expr.is_some() {
                        return Err(format!(
                            "streaming `with` selector cannot decorate a nested concatenation in `{path}`"
                        ));
                    }
                    if reordered {
                        operands.reverse();
                    }
                    pending.extend(operands.into_iter().rev());
                    continue;
                }

                if let Some(with_node) = stream.with_expr {
                    if let Some(bounds) = self.fixed_with_bounds(path, value)? {
                        if !self.static_with_in_bounds(path, with_node, bounds)? {
                            // SV 11.4.14.4: a later selector observes values
                            // unpacked to its left. A queued unpack has not
                            // published them, and a `<<` unpack must size its
                            // consumed source before reordering it, so either
                            // form rejects that dependence.
                            if (!blocking || reversed_fixed)
                                && self.reads_overlap_lvalue_writes(path, with_node, &earlier)?
                            {
                                let form = match mode {
                                    StreamTargetMode::Blocking => "right-to-left",
                                    StreamTargetMode::Nonblocking => "nonblocking",
                                    StreamTargetMode::CopyOut => "output copy-out",
                                };
                                return Err(format!(
                                    "{form} streaming `with` selector reads a target unpacked earlier by the same assignment in `{path}`"
                                ));
                            }
                            let selector = self.lower_stream_selector(path, with_node)?;
                            if let Some(array) = self.array_of(value).cloned() {
                                if array.real {
                                    return Err(format!(
                                        "real array streaming assignment target is not supported in `{path}`"
                                    ));
                                }
                                targets.push(IrStreamTarget::FixedSelector {
                                    array: array.ir,
                                    selector,
                                });
                            } else {
                                let (target, shape) = self.fixed_image_target(path, value)?;
                                targets.push(IrStreamTarget::FixedImageSelector {
                                    target: Box::new(target),
                                    bounds: shape.bounds,
                                    element_width: shape.element_width,
                                    two_state: shape.two_state,
                                    selector,
                                    two_state_runs: shape.two_state_runs,
                                });
                            }
                            earlier.push(value);
                            continue;
                        }
                    } else if self.array_of(value).is_some() {
                        return Err(Self::multidimensional_with_error(path));
                    }
                }

                if let Some(fixed_parts) =
                    self.fixed_stream_lhs_parts(path, value, stream.with_expr)?
                {
                    for part in fixed_parts {
                        let part = self.lhs_to_ir(part)?;
                        let width = packed_lhs_width(&self.model, &part).ok_or_else(|| {
                            format!(
                                "streaming assignment target must be a packed lvalue in `{path}`"
                            )
                        })?;
                        targets.push(IrStreamTarget::Packed { lhs: part, width });
                    }
                    earlier.push(value);
                    continue;
                }

                if let Some(container) = self.container_of(value) {
                    container_count += 1;
                    if container_count > 1 {
                        return Err(format!(
                            "streaming assignment supports at most one resizable target in `{path}`"
                        ));
                    }
                    let target = self.model.containers.get(container.ir).ok_or_else(|| {
                        format!("streaming target container is out of bounds in `{path}`")
                    })?;
                    if !matches!(
                        target.kind,
                        IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                    ) {
                        return Err(format!(
                            "associative arrays are not legal streaming targets in `{path}`"
                        ));
                    }
                    if target.element.packed().is_none() {
                        return Err(format!(
                            "streaming target must have a packed element in `{path}`"
                        ));
                    }
                    let selector = stream
                        .with_expr
                        .map(|node| self.lower_stream_selector(path, node))
                        .transpose()?;
                    targets.push(IrStreamTarget::Container {
                        container: container.ir,
                        selector,
                    });
                    earlier.push(value);
                    continue;
                }

                if self.stream_target_contains_container(value) {
                    return Err(format!(
                        "nested resizable containers are not legal streaming targets in `{path}`"
                    ));
                }
                if stream.with_expr.is_some() {
                    return Err(format!(
                        "streaming `with` selector requires a one-dimensional unpacked array target in `{path}`"
                    ));
                }
                if matches!(self.kind(value), NodeKind::Expr(ExprKind::Streaming { .. })) {
                    return Err(format!(
                        "nested streaming targets are not supported in `{path}`"
                    ));
                }
                let analyzed = self.analyze_lhs(path, value)?;
                let part = self.lhs_to_ir(analyzed)?;
                let width = packed_lhs_width(&self.model, &part).ok_or_else(|| {
                    format!("streaming assignment target must be a packed lvalue in `{path}`")
                })?;
                targets.push(IrStreamTarget::Packed { lhs: part, width });
                earlier.push(value);
            }
        }
        if targets.is_empty() {
            return Err(format!("empty streaming assignment target in `{path}`"));
        }

        let slice = if slice_size == 0 {
            1
        } else {
            u32::try_from(slice_size)
                .map_err(|_| format!("streaming slice size is too large in `{path}`"))?
        };
        Ok(Some(StreamTargetPlan {
            targets,
            slice,
            direction: match direction {
                DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
            },
        }))
    }

    /// Lower a direct streaming target backed by a packed-element dynamic
    /// array or queue.  The runtime operation owns source materialization and
    /// target resizing, which keeps overlapping source/destination updates
    /// atomic and gives dynamic selectors one evaluation point.
    pub(super) fn lower_stream_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let NodeKind::Expr(ExprKind::Streaming {
            direction,
            slice_size,
            streams,
        }) = self.kind(lhs)
        else {
            return Ok(None);
        };
        if !streams
            .iter()
            .any(|stream| self.stream_target_contains_container(stream.value))
        {
            return Ok(None);
        }
        if !blocking {
            return Err(format!(
                "nonblocking assignment to a streaming container target in `{path}` is not supported"
            ));
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to a streaming container target in `{path}` is not supported"
            ));
        }
        if streams.len() != 1 {
            return Ok(None);
        }
        let stream = &streams[0];
        let Some(container) = self.container_of(stream.value) else {
            return Ok(None);
        };
        let target =
            self.model.containers.get(container.ir).ok_or_else(|| {
                format!("streaming container target is out of bounds in `{path}`")
            })?;
        if !matches!(
            target.kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "associative arrays are not legal streaming targets in `{path}`"
            ));
        }
        if target.element.packed().is_none() {
            return Err(format!(
                "streaming container target must have a packed element in `{path}`"
            ));
        }
        let selector = stream
            .with_expr
            .map(|node| self.lower_stream_selector(path, node))
            .transpose()?;
        let source = self.lower_stream_operand(path, rhs, None)?;
        if source.is_real() {
            return Err(format!(
                "real source is not legal for a streaming container target in `{path}`"
            ));
        }
        let slice = if *slice_size == 0 {
            1
        } else {
            u32::try_from(*slice_size)
                .map_err(|_| format!("streaming slice size is too large in `{path}`"))?
        };
        Ok(Some(IrStmt::Container(Box::new(
            IrContainerStmt::StreamAssign {
                container: container.ir,
                source,
                slice,
                direction: match direction {
                    DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                    DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                },
                selector,
            },
        ))))
    }
}

/// Evaluation point of a checked streaming unpack's `with` selectors.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum StreamTargetMode {
    /// Selectors are evaluated in stream order as each target is written.
    Blocking,
    /// Selectors are evaluated at issue; writes are queued.
    Nonblocking,
    /// Output copy-out: selectors are frozen with the actual at the call,
    /// before the callee runs, and the targets are written after it returns.
    CopyOut,
}

/// Fixed and resizable components of a checked streaming unpack.
pub(in super::super) struct StreamTargetPlan {
    pub(in super::super) targets: Vec<IrStreamTarget>,
    pub(in super::super) slice: u32,
    pub(in super::super) direction: IrStreamDirection,
}

impl StreamTargetPlan {
    pub(in super::super) fn assign(self, source: IrExpr, nba: bool) -> IrStmt {
        IrStmt::StreamAssign {
            source,
            slice: self.slice,
            direction: self.direction,
            targets: self.targets,
            nba,
        }
    }
}
