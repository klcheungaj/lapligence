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
        statements.push(IrStmt::Container(IrContainerStmt::AssignValues {
            container: dst.ir,
            values,
        }));
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

    /// Lower a streaming assignment whose target contains both ordinary
    /// packed lvalues and one packed-element resizable container. The source
    /// is kept as one packed value and the emitter consumes its materialized
    /// stream in target order, so all source reads happen before any writes.
    pub(super) fn lower_stream_mixed_assignment(
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
        let mut needs_mixed = false;
        for stream in streams {
            if self.stream_target_contains_container(stream.value) {
                needs_mixed = true;
                break;
            }
            if let Some(with_node) = stream.with_expr {
                if self.array_of(stream.value).is_some()
                    && self
                        .static_stream_selector_indices(path, with_node)?
                        .is_none()
                {
                    needs_mixed = true;
                    break;
                }
            }
        }
        if !needs_mixed {
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

        let streams = streams.clone();
        let mut targets = Vec::new();
        let mut container_count = 0usize;
        for stream in streams {
            if stream.with_expr.is_some()
                && self.array_of(stream.value).is_none()
                && self.container_of(stream.value).is_none()
            {
                return Err(format!(
                    "streaming `with` selector requires a packed-element array target in `{path}`"
                ));
            }

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
                    if let Some(array) = self.array_of(value).cloned() {
                        if array.real {
                            return Err(format!(
                                "real array streaming assignment target is not supported in `{path}`"
                            ));
                        }
                        if self
                            .static_stream_selector_indices(path, with_node)?
                            .is_none()
                        {
                            if array.dims.len() != 1 {
                                return Err(format!(
                                    "runtime `with` selector on a multidimensional fixed streaming target is not supported in `{path}`"
                                ));
                            }
                            let selector = self.lower_stream_selector(path, with_node)?;
                            targets.push(IrStreamTarget::FixedSelector {
                                array: array.ir,
                                selector,
                            });
                            continue;
                        }
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
                    continue;
                }

                if self.stream_target_contains_container(value) {
                    return Err(format!(
                        "nested resizable containers are not legal streaming targets in `{path}`"
                    ));
                }
                if stream.with_expr.is_some() {
                    return Err(format!(
                        "streaming `with` selector requires a packed-element array target in `{path}`"
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
            }
        }
        if targets.is_empty() {
            return Err(format!("empty streaming assignment target in `{path}`"));
        }

        let source = self.lower_stream_operand(path, rhs, None)?;
        if source.is_real() {
            return Err(format!(
                "real source is not legal for a streaming assignment in `{path}`"
            ));
        }
        let slice = if *slice_size == 0 {
            1
        } else {
            u32::try_from(*slice_size)
                .map_err(|_| format!("streaming slice size is too large in `{path}`"))?
        };
        Ok(Some(IrStmt::StreamAssign {
            source,
            slice,
            direction: match direction {
                DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
            },
            targets,
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
        Ok(Some(IrStmt::Container(IrContainerStmt::StreamAssign {
            container: container.ir,
            source,
            slice,
            direction: match direction {
                DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
            },
            selector,
        })))
    }
}
