//! Assignments.

use super::super::expressions::ExpandedConditional;
use super::*;

/// `dst = c ? a : b` for dynamic arrays (including fixed-array views of
/// native elements), queues and associative arrays.
struct ContainerConditional {
    type_node: NodeId,
    dst: usize,
}

impl ExpandedConditional for ContainerConditional {
    type Operand = usize;

    fn nested(
        &mut self,
        cg: &mut Codegen<'_>,
        _path: &str,
        arm: NodeId,
    ) -> Result<Option<NodeId>, String> {
        // Any other conversion of an alternative has a lowering of its own.
        let arm = cg.class_array_upcast_operand(arm).unwrap_or(arm);
        Ok(cg.is_conditional(arm).then_some(arm))
    }

    fn assign(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        dst: Option<&usize>,
        arm: NodeId,
    ) -> Result<IrStmt, String> {
        cg.lower_container_into(path, self.type_node, *dst.unwrap_or(&self.dst), arm)
    }

    fn evaluate(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        arm: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<usize, String> {
        let value = self.declare_kept(cg, arm, out)?;
        out.push(self.assign(cg, path, Some(&value), arm)?);
        Ok(value)
    }

    fn reads_in_place(&mut self, _cg: &mut Codegen<'_>, _arm: NodeId) -> Result<bool, String> {
        Ok(false)
    }

    fn declare_kept(
        &mut self,
        cg: &mut Codegen<'_>,
        _node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<usize, String> {
        let temporary = cg.container_temporary_like(self.dst);
        out.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
            temporary,
        ))));
        Ok(temporary)
    }

    fn merge(
        &mut self,
        _cg: &mut Codegen<'_>,
        _path: &str,
        dst: Option<&usize>,
        left: &usize,
        right: &usize,
    ) -> Result<Vec<IrStmt>, String> {
        Ok(vec![IrStmt::Container(Box::new(IrContainerStmt::Merge {
            dst: *dst.unwrap_or(&self.dst),
            left: *left,
            right: *right,
        }))])
    }

    fn private_target(&self, _cg: &Codegen<'_>) -> Option<usize> {
        None
    }

    fn copy(
        &mut self,
        _cg: &mut Codegen<'_>,
        _path: &str,
        dst: Option<&usize>,
        src: &usize,
    ) -> Result<IrStmt, String> {
        Ok(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: *dst.unwrap_or(&self.dst),
            src: *src,
        })))
    }
}

impl<'a> Codegen<'a> {
    pub(in super::super) fn lower_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        // A staged nested element (`sq[k] = src`) is a whole container.
        if let Some(staged) = self.staged_nested_element(lhs) {
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to nested resizable container element in {path} is illegal"
                ));
            }
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to nested resizable container element in {path} is not supported"
                ));
            }
            return self.lower_container_into(path, lhs, staged, rhs).map(Some);
        }
        if let Some(statement) =
            self.lower_stream_container_assignment(path, lhs, rhs, blocking, op)?
        {
            return Ok(Some(statement));
        }
        if let Some(statement) =
            self.lower_stream_into_fixed_descriptor(path, lhs, rhs, blocking, op)?
        {
            return Ok(Some(statement));
        }
        if let Some(statement) = self.lower_stream_mixed_assignment(path, lhs, rhs, blocking, op)? {
            return Ok(Some(statement));
        }
        if let Some(statement) =
            self.lower_container_select_assignment(path, lhs, rhs, blocking, op)?
        {
            return Ok(Some(statement));
        }
        if !blocking {
            if op != Operation::Assignment {
                return Err(format!(
                    "compound nonblocking assignment in `{path}` is illegal"
                ));
            }
            if let Some(statement) = self.lower_fixed_view_nba(path, lhs, rhs, None)? {
                return Ok(Some(statement));
            }
        }
        if self.p30_fixed_array_assignment_candidate(lhs) {
            return self.lower_p30_fixed_array_assignment(path, lhs, rhs, blocking, op);
        }
        if self.is_container_record(lhs) {
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to resizable container element in `{path}` is illegal"
                ));
            }
            if op != Operation::Assignment {
                return Err(format!(
                    "compound assignment to a record container element in `{path}` is not supported"
                ));
            }
            return self.lower_container_record_assignment(path, lhs, rhs);
        }
        if let Some((container, source_indices)) = self.container_element_path(lhs) {
            if source_indices.len() == 1
                && self
                    .container_element_type(container, 1)
                    .is_some_and(|element| matches!(element, IrContainerElement::Container { .. }))
            {
                if !blocking {
                    return Err(format!(
                        "nonblocking assignment to nested resizable container element in {path} is illegal"
                    ));
                }
                if op != Operation::Assignment {
                    return Err(format!(
                        "compound assignment to nested resizable container element in {path} is not supported"
                    ));
                }
                let indices = source_indices
                    .into_iter()
                    .map(|index| self.lower_container_index(path, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut prelude = Vec::new();
                let source = self.nested_container_source(path, container, 1, rhs, &mut prelude)?;
                prelude.push(IrStmt::Container(Box::new(IrContainerStmt::SetContainer {
                    container,
                    indices,
                    source,
                })));
                return Ok(Some(IrStmt::Block(prelude)));
            }
            if source_indices.len() > 1 {
                if !blocking {
                    return Err(format!(
                        "nonblocking assignment to nested resizable container element in {path} is illegal"
                    ));
                }
                if op != Operation::Assignment {
                    return Err(format!(
                        "compound assignment to nested resizable container element in {path} is not supported"
                    ));
                }
                let indices = source_indices
                    .into_iter()
                    .map(|index| self.lower_container_index(path, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let element = self
                    .container_element_type(container, indices.len())
                    .ok_or_else(|| format!("invalid nested container write in {path}"))?;
                let operation = match &element {
                    IrContainerElement::Packed { .. } => IrContainerStmt::SetNested {
                        container,
                        indices,
                        value: self.lower_container_value_for_element(path, &element, rhs)?,
                    },
                    IrContainerElement::Real { .. } => IrContainerStmt::SetNestedReal {
                        container,
                        indices,
                        value: self.lower_expr(path, rhs)?,
                    },
                    IrContainerElement::String => IrContainerStmt::SetNestedString {
                        container,
                        indices,
                        value: self.lower_string(path, rhs)?,
                    },
                    element if element.is_handle() => IrContainerStmt::SetNestedChandle {
                        container,
                        indices,
                        value: self.lower_container_handle(path, element, rhs)?,
                    },
                    IrContainerElement::Container { .. } => {
                        let source = self.container_of(rhs).ok_or_else(|| {
                            format!("nested container assignment in {path} requires a dynamic array")
                        })?;
                        if !matches!(
                            self.model.containers[source.ir].kind,
                            IrContainerKind::Dynamic
                        ) {
                            return Err(format!(
                                "nested container assignment in {path} requires a dynamic array"
                            ));
                        }
                        IrContainerStmt::SetContainer {
                            container,
                            indices,
                            source: source.ir,
                        }
                    }
                    _ => {
                        return Err(format!(
                            "nested resizable container write in {path} has an unsupported element type"
                        ))
                    }
                };
                return Ok(Some(IrStmt::Container(Box::new(operation))));
            }
        }
        let selected = self.element_select(lhs);
        if let Some((container, index)) = selected {
            if !blocking {
                return Err(format!(
                    "nonblocking assignment to resizable container element in `{path}` is illegal"
                ));
            }
            if op != Operation::Assignment {
                let mut rhs_value = Some(self.lower_expr(path, rhs)?);
                return self
                    .lower_container_element_update(path, lhs, &mut |path, current| {
                        let rhs_value = rhs_value
                            .take()
                            .ok_or("compound element update evaluated twice")?;
                        crate::sim::codegen::lowering::lower_compound_expr_ir(
                            path, op, current, rhs_value,
                        )
                    })
                    .map(Some);
            }
            if self
                .container_element_type(container.ir, 1)
                .is_some_and(|element| matches!(element, IrContainerElement::Container { .. }))
            {
                let indices = vec![self.lower_container_index(path, index)?];
                let mut prelude = Vec::new();
                let source =
                    self.nested_container_source(path, container.ir, 1, rhs, &mut prelude)?;
                prelude.push(IrStmt::Container(Box::new(IrContainerStmt::SetContainer {
                    container: container.ir,
                    indices,
                    source,
                })));
                return Ok(Some(IrStmt::Block(prelude)));
            }
            let operation = match self.model.containers[container.ir].kind {
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => {
                    let key = self.lower_string(path, index)?;
                    match self.model.containers[container.ir].element.clone() {
                        IrContainerElement::Packed { .. } => IrContainerStmt::SetString {
                            container: container.ir,
                            key,
                            value: self.lower_container_value(path, container.ir, rhs)?,
                        },
                        IrContainerElement::Real { .. } => IrContainerStmt::SetStringReal {
                            container: container.ir,
                            key,
                            value: self.lower_expr(path, rhs)?,
                        },
                        IrContainerElement::String => IrContainerStmt::SetStringString {
                            container: container.ir,
                            key,
                            value: self.lower_string(path, rhs)?,
                        },
                        ref element if element.is_handle() => IrContainerStmt::SetStringChandle {
                            container: container.ir,
                            key,
                            value: self.lower_container_handle(path, element, rhs)?,
                        },
                        _ => {
                            return Err(format!(
                                "string-keyed associative element write in `{path}` requires a scalar value"
                            ))
                        }
                    }
                }
                _ => {
                    let index = self.lower_container_top_index(path, container.ir, index)?;
                    match self.model.containers[container.ir].element.clone() {
                        IrContainerElement::Packed { .. } => IrContainerStmt::Set {
                            container: container.ir,
                            index,
                            value: self.lower_container_value(path, container.ir, rhs)?,
                        },
                        IrContainerElement::Real { .. } => IrContainerStmt::SetReal {
                            container: container.ir,
                            index,
                            value: self.lower_expr(path, rhs)?,
                        },
                        IrContainerElement::String => IrContainerStmt::SetStringValue {
                            container: container.ir,
                            index,
                            value: self.lower_string(path, rhs)?,
                        },
                        ref element if element.is_handle() => IrContainerStmt::SetChandleValue {
                            container: container.ir,
                            index,
                            value: self.lower_container_handle(path, element, rhs)?,
                        },
                        IrContainerElement::Container { .. } => {
                            let source = self.container_of(rhs).ok_or_else(|| {
                                format!(
                                    "nested container assignment in {path} requires a dynamic array"
                                )
                            })?;
                            if !matches!(
                                self.model.containers[source.ir].kind,
                                IrContainerKind::Dynamic
                            ) {
                                return Err(format!(
                                    "nested container assignment in {path} requires a dynamic array"
                                ));
                            }
                            IrContainerStmt::SetContainer {
                                container: container.ir,
                                indices: vec![index],
                                source: source.ir,
                            }
                        }
                        _ => {
                            return Err(format!(
                                "resizable container element write in `{path}` requires a directly represented scalar element: {:?}",
                                self.model.containers[container.ir].element
                            ))
                        }
                    }
                }
            };
            return Ok(Some(IrStmt::Container(Box::new(operation))));
        }

        if let Some((dst, dst_start, count)) = self.fixed_view_slice(path, lhs)? {
            if !blocking || op != Operation::Assignment {
                return Err(format!(
                    "nonblocking or compound assignment to a fixed-array slice of native elements in `{path}` is not supported"
                ));
            }
            // The source is the whole right-hand array, or a slice of one;
            // any other source (a pattern, call or conditional) is built in
            // a temporary of the selection's own shape first.
            let mut statements = Vec::new();
            let (src, src_start) = match self.fixed_view_slice(path, rhs)? {
                Some((src, start, _)) => (src, start),
                None => match self.container_of(self.p30_unwrap_cast(rhs)) {
                    Some(src) => (src.ir, pattern_key_expr(0, 64, true, false)),
                    // Packed cells (a multidimensional class property) take
                    // the source's values directly.
                    None if self.model.containers[dst].element.is_packed() => {
                        return self
                            .lower_packed_row_write(path, lhs, rhs, dst, dst_start, count)
                            .map(Some);
                    }
                    None => {
                        let temporary = self.selection_temporary(dst, lhs, count);
                        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                            temporary,
                        ))));
                        statements.push(self.lower_container_into(path, lhs, temporary, rhs)?);
                        (temporary, pattern_key_expr(0, 64, true, false))
                    }
                },
            };
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::CopyRange {
                dst,
                dst_start,
                src,
                src_start,
                count,
            })));
            return Ok(Some(if statements.len() == 1 {
                statements.remove(0)
            } else {
                IrStmt::Block(statements)
            }));
        }
        let Some(dst) = self.container_of(lhs) else {
            return Ok(None);
        };
        if !blocking {
            return Err(format!(
                "nonblocking assignment to resizable container in `{path}` is illegal"
            ));
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to resizable container in `{path}` is not supported"
            ));
        }
        self.lower_container_into(path, lhs, dst.ir, rhs).map(Some)
    }

    /// Replace the whole of container `dst` with `rhs`. `type_node` carries
    /// the target's declared type (the assignment target, or the formal a
    /// call temporary stands for).
    pub(in super::super) fn lower_container_into(
        &mut self,
        path: &str,
        type_node: NodeId,
        dst: usize,
        rhs: NodeId,
    ) -> Result<IrStmt, String> {
        self.lower_container_into_unlocated(path, type_node, dst, rhs)
            .map_err(|error| self.locate_error(rhs, error))
    }

    /// [`Self::lower_container_into`] before its failure is located.
    fn lower_container_into_unlocated(
        &mut self,
        path: &str,
        type_node: NodeId,
        dst: usize,
        rhs: NodeId,
    ) -> Result<IrStmt, String> {
        let dst = ContainerInfo { ir: dst };
        let rhs = self.class_array_upcast_operand(rhs).unwrap_or(rhs);
        if let Some(statement) = self.lower_container_result_into(path, rhs, dst.ir)? {
            return Ok(statement);
        }
        // `{}` is the empty unpacked array concatenation (SV 7.10.4): every
        // element is removed, exactly as by `delete()`.
        if matches!(
            self.kind(self.p30_unwrap_cast(rhs)),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) if operands.is_empty()
        ) && !matches!(
            self.model.containers[dst.ir].kind,
            IrContainerKind::Associative { .. }
        ) {
            return Ok(IrStmt::Container(Box::new(IrContainerStmt::Delete(dst.ir))));
        }
        // A streaming concatenation resizes a dynamic array or queue to hold
        // the whole stream, left-aligned and zero-filled (SV 11.4.14).
        if let Some(statement) = self.lower_stream_into_container(path, dst.ir, rhs)? {
            return Ok(statement);
        }
        // A dynamic array and a queue of one element type assign to each
        // other through an implicit conversion (SV 7.6), which is a whole
        // copy, not a bit-stream cast. An explicit cast between different
        // element widths is a bit-stream cast instead.
        if let Some(source) = self
            .container_of(self.p30_unwrap_cast(rhs))
            .filter(|source| source.ir != dst.ir)
        {
            let (target, origin) = (
                &self.model.containers[dst.ir],
                &self.model.containers[source.ir],
            );
            if matches!(
                (&target.kind, &origin.kind),
                (IrContainerKind::Dynamic, IrContainerKind::Queue { .. })
                    | (IrContainerKind::Queue { .. }, IrContainerKind::Dynamic)
            ) && target.element.compatible_with(&origin.element)
                && target.element.is_packed() == origin.element.is_packed()
                && target.element.packed().map(|(width, _, _)| width)
                    == origin.element.packed().map(|(width, _, _)| width)
            {
                return Ok(IrStmt::Container(Box::new(IrContainerStmt::Copy {
                    dst: dst.ir,
                    src: source.ir,
                })));
            }
        }
        // A row or slice converted implicitly (`d = o.g[i]`) is an element
        // copy, not a bit-stream cast.
        if matches!(self.kind(rhs), NodeKind::Expr(ExprKind::Cast { .. }))
            && self.db.is_implicit_conversion(rhs)
            && self.is_fixed_view_slice(rhs)
        {
            if let Some((src, start, count)) = self.fixed_view_slice(path, rhs)? {
                return self.lower_slice_into(path, dst.ir, src, start, count);
            }
        }
        // A whole fixed array of strings, handles or records converted
        // implicitly to a dynamic array (`d = t`, SV 7.6) is an element copy
        // too; a bit-stream cast needs packed elements.
        if matches!(self.kind(rhs), NodeKind::Expr(ExprKind::Cast { .. }))
            && self.db.is_implicit_conversion(rhs)
            && !self.model.containers[dst.ir].element.is_packed()
        {
            let whole = self
                .container_of(self.p30_unwrap_cast(rhs))
                .map(|source| source.ir)
                .filter(|source| *source != dst.ir && self.is_fixed_view(*source))
                .and_then(|source| {
                    let origin = &self.model.containers[source];
                    let target = &self.model.containers[dst.ir];
                    (target.element.compatible_with(&origin.element))
                        .then_some(origin.initial_size)
                        .flatten()
                        .map(|count| (source, count))
                });
            if let Some((source, count)) = whole {
                return self.lower_slice_into(
                    path,
                    dst.ir,
                    source,
                    pattern_key_expr(0, 64, true, false),
                    count,
                );
            }
        }
        if let Some(statement) =
            self.lower_bitstream_cast_container_assignment(path, type_node, rhs, &dst)?
        {
            return Ok(statement);
        }
        let descriptor = self.query_descriptor(type_node).cloned();
        if self.assignment_pattern_operands(path, rhs)?.is_some() {
            return self.lower_container_pattern(path, dst.ir, rhs, descriptor.as_ref());
        }
        if let Some(statement) = self.nested_pop_into(dst.ir, rhs) {
            return Ok(statement);
        }
        if let Some(statement) = self.container_method_result(path, dst.ir, rhs)? {
            return Ok(statement);
        }
        let new_array = match self.kind(rhs) {
            NodeKind::Expr(ExprKind::NewArray { size, initializer }) => Some((*size, *initializer)),
            _ => None,
        };
        if let Some((size, initializer)) = new_array {
            if !matches!(self.model.containers[dst.ir].kind, IrContainerKind::Dynamic) {
                return Err(format!("new[] target in `{path}` is not a dynamic array"));
            }
            // The size takes the 64 signed bits of a longint (SV 7.5.1) but
            // stays four-state: the runtime size check reports X or Z bits
            // where a two-state conversion would size the array from 0s
            // (S07-D17).
            let size = ir_to_storage(self.lower_expr(path, size)?, 64, true, false)?;
            let initializer = initializer
                .map(|node| {
                    self.container_of(node)
                        .filter(|source| {
                            matches!(
                                self.model.containers[source.ir].kind,
                                IrContainerKind::Dynamic
                            )
                        })
                        .map(|source| source.ir)
                        .ok_or_else(|| {
                            format!(
                                "dynamic-array new[] initializer in `{path}` must be a compatible dynamic array"
                            )
                        })
                })
                .transpose()?;
            return Ok(IrStmt::Container(Box::new(IrContainerStmt::DynamicNew {
                container: dst.ir,
                size,
                initializer,
            })));
        }
        if let Some((src, start, count)) = self.fixed_view_slice(path, rhs)? {
            return self.lower_slice_into(path, dst.ir, src, start, count);
        }
        // A known predicate assigns one alternative; an ambiguous one
        // evaluates both into temporaries and merges them leaf by leaf
        // (SV 11.4.11, see [`IrContainerStmt::Merge`]).
        let source = self.p30_unwrap_cast(rhs);
        if self.is_conditional(source) {
            let mut sink = ContainerConditional {
                type_node,
                dst: dst.ir,
            };
            return self.expand_conditional(path, &mut sink, source);
        }
        if matches!(
            self.model.containers[dst.ir].kind,
            IrContainerKind::Queue { .. }
        ) {
            if let Some(sources) = self.lower_queue_sources(path, rhs)? {
                return Ok(IrStmt::Container(Box::new(IrContainerStmt::QueueAssign {
                    container: dst.ir,
                    sources,
                })));
            }
        }
        if let Some(statement) = self.lower_container_concat_into(path, dst.ir, rhs)? {
            return Ok(statement);
        }
        let Some(src) = self.container_of(rhs) else {
            // A whole class property of integral cells (`h.g = r2`) takes
            // a fixed-array value cell for cell, as one of its rows does.
            if self.is_fixed_view(dst.ir) && self.model.containers[dst.ir].element.is_packed() {
                if let Some(count) = self.model.containers[dst.ir].initial_size {
                    return self.lower_packed_row_write(
                        path,
                        type_node,
                        rhs,
                        dst.ir,
                        pattern_key_expr(0, 64, true, false),
                        count,
                    );
                }
            }
            self.reject_native_row_copy(path, rhs)?;
            return Err(format!(
                "resizable container assignment in `{path}` requires a compatible array (got {:?})",
                self.kind(rhs)
            ));
        };
        Ok(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: dst.ir,
            src: src.ir,
        })))
    }

    /// Items of an unpacked array concatenation in order, nested
    /// concatenations flattened (SV 10.10).
    fn concat_items(&self, node: NodeId, items: &mut Vec<NodeId>) -> bool {
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Concat,
            operands,
            reordered,
            ..
        }) = self.kind(self.p30_unwrap_cast(node))
        else {
            return false;
        };
        let mut nested = Vec::new();
        for operand in operands {
            if !self.concat_items(*operand, &mut nested) {
                nested.push(*operand);
            }
        }
        if *reordered {
            nested.reverse();
        }
        items.extend(nested);
        true
    }

    /// `{a, q, b}` (SV 10.10): an unpacked array concatenation of element
    /// values, and for a queue target also of queues and queue slices, into
    /// a queue or dynamic array. Element values are evaluated before the
    /// target changes; each run of them fills a temporary queue source.
    fn lower_container_concat_into(
        &mut self,
        path: &str,
        dst: usize,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let mut items = Vec::new();
        if !self.concat_items(rhs, &mut items) || items.is_empty() {
            return Ok(None);
        }
        if matches!(
            self.model.containers[dst].kind,
            IrContainerKind::Associative { .. }
        ) {
            return Ok(None);
        }
        let mut array_items = Vec::with_capacity(items.len());
        for item in &items {
            let array = self.container_of(self.p30_unwrap_cast(*item)).is_some()
                || matches!(
                    self.kind(self.p30_unwrap_cast(*item)),
                    NodeKind::Expr(ExprKind::PartSelect { .. })
                ) && self.lower_queue_sources(path, *item)?.is_some();
            array_items.push(array);
        }
        if !array_items.iter().any(|array| *array) {
            return self
                .lower_container_source_values(path, dst, items)
                .map(Some);
        }
        if !matches!(
            self.model.containers[dst].kind,
            IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "unpacked array concatenation of arrays into a dynamic array in `{path}` is not supported; assign it to a queue"
            ));
        }
        let mut statements = Vec::new();
        let mut sources = Vec::new();
        let mut run = Vec::new();
        for (item, array) in items.into_iter().zip(array_items) {
            if !array {
                run.push(item);
                continue;
            }
            if !run.is_empty() {
                sources.push(self.concat_run_source(
                    path,
                    dst,
                    std::mem::take(&mut run),
                    &mut statements,
                )?);
            }
            let Some(mut queue_sources) = self.lower_queue_sources(path, item)? else {
                return Err(format!(
                    "unpacked array concatenation item in `{path}` must be an element value, a queue or a queue slice"
                ));
            };
            sources.append(&mut queue_sources);
        }
        if !run.is_empty() {
            sources.push(self.concat_run_source(path, dst, run, &mut statements)?);
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::QueueAssign {
            container: dst,
            sources,
        })));
        Ok(Some(IrStmt::Block(statements)))
    }

    /// A temporary queue holding one run of concatenation element values.
    fn concat_run_source(
        &mut self,
        path: &str,
        dst: usize,
        values: Vec<NodeId>,
        statements: &mut Vec<IrStmt>,
    ) -> Result<IrQueueSource, String> {
        let temporary = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{temporary}"),
            element: self.model.containers[dst].element.clone(),
            kind: IrContainerKind::Queue {
                maximum_elements: None,
            },
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        // Record elements take their descriptor from the target's type.
        self.container_types_like.insert(temporary, dst);
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
            temporary,
        ))));
        statements.push(self.lower_container_source_values(path, temporary, values)?);
        Ok(IrQueueSource::Whole(temporary))
    }

    /// Replace container `dst` with a slice of a fixed-array view. A
    /// dynamic-array destination takes the slice size first; the runtime
    /// snapshots the source before writing.
    /// An activation container shaped like the selection `node` of view
    /// `view` (`count` cells): the view's own shape when `node` is the whole
    /// view, its declared row dimensions when `node` is a row of a
    /// multidimensional view, else one dimension from 0.
    pub(in super::super) fn selection_temporary(
        &mut self,
        view: usize,
        node: NodeId,
        count: u64,
    ) -> usize {
        let temporary = self.container_temporary_like(view);
        if self
            .container_of(self.p30_unwrap_cast(node))
            .is_some_and(|whole| whole.ir == view)
        {
            return temporary;
        }
        self.model.containers[temporary].initial_size = Some(count);
        self.fixed_view_ranges.remove(&temporary);
        self.fixed_view_dims.remove(&temporary);
        let rows = match self.kind(self.p30_unwrap_cast(node)) {
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => self
                .fixed_view_dims
                .get(&view)
                .filter(|dimensions| indices.len() < dimensions.len())
                .map(|dimensions| dimensions[indices.len()..].to_vec()),
            _ => None,
        };
        match rows {
            Some(rows) if rows.len() > 1 => {
                self.fixed_view_dims.insert(temporary, rows);
            }
            Some(rows) => {
                self.fixed_view_ranges.insert(temporary, rows[0]);
            }
            None => {
                let last = i32::try_from(count.saturating_sub(1)).unwrap_or(i32::MAX);
                self.fixed_view_ranges.insert(temporary, (0, last));
            }
        }
        temporary
    }

    fn lower_slice_into(
        &mut self,
        path: &str,
        dst: usize,
        src: usize,
        start: IrExpr,
        count: u64,
    ) -> Result<IrStmt, String> {
        // Packed cells copy only between packed arrays (a row of a
        // multidimensional class property, SIM-011).
        if !matches!(self.model.containers[dst].kind, IrContainerKind::Dynamic)
            || self.model.containers[dst].element.is_packed()
                != self.model.containers[src].element.is_packed()
        {
            return Err(format!(
                "fixed-array slice in `{path}` can only be assigned to a fixed or dynamic array"
            ));
        }
        let mut statements = Vec::new();
        if self.model.containers[dst].initial_size != Some(count) {
            let temporary = self.container_temporary_like(dst);
            self.model.containers[temporary].initial_size = Some(count);
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                temporary,
            ))));
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::CopyRange {
                dst: temporary,
                dst_start: pattern_key_expr(0, 64, true, false),
                src,
                src_start: start,
                count,
            })));
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
                dst,
                src: temporary,
            })));
            return Ok(IrStmt::Block(statements));
        }
        Ok(IrStmt::Container(Box::new(IrContainerStmt::CopyRange {
            dst,
            dst_start: pattern_key_expr(0, 64, true, false),
            src,
            src_start: start,
            count,
        })))
    }

    /// `dst = q.pop_front()` over a queue of containers: the end element is
    /// read into `dst` and then removed, so an empty queue leaves `dst` empty
    /// (SV 7.10.2.4) and reports the removal. `None` for other sources.
    fn nested_pop_into(&mut self, dst: usize, rhs: NodeId) -> Option<IrStmt> {
        let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } = self.kind(rhs)
        else {
            return None;
        };
        let back = match name.as_str() {
            "pop_front" => false,
            "pop_back" => true,
            _ => return None,
        };
        let queue = self.container_of(*receiver)?.ir;
        let nested = self.container_element_type(queue, 1)?;
        let (IrContainerElement::Container { element, .. }, Some(kind)) =
            (nested.clone(), nested.nested_kind())
        else {
            return None;
        };
        let index = if back {
            let size = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Size(queue))),
                32,
                true,
                None,
            );
            crate::sim::codegen::lowering::common_bin_expr(
                IrBinOp::Sub,
                size,
                pattern_key_expr(1, 32, true, true),
            )
        } else {
            pattern_key_expr(0, 32, true, true)
        };
        let root = |write: bool| {
            crate::sim::ir::IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
                container: queue,
                indices: vec![index.clone()],
                key: None,
                write,
            })
        };
        let alias = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{alias}"),
            element: *element,
            kind,
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        let mut statements = vec![
            IrStmt::Container(Box::new(IrContainerStmt::BindElement {
                root: root(false),
                items: Vec::new(),
                container: alias,
            })),
            IrStmt::Container(Box::new(IrContainerStmt::Copy { dst, src: alias })),
        ];
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::DeleteIndex {
            container: queue,
            index,
        })));
        Some(IrStmt::Block(statements))
    }

    /// Read-modify-write of one packed or real element selected by a single
    /// index (`c[i] op= v`, `c[i]++`). The index is evaluated once for the
    /// read and once for the write, so it must be free of side effects. A
    /// nonexistent associative entry reads its default (SV 7.8.6) and the
    /// write then creates it.
    pub(in super::super) fn lower_container_element_update(
        &mut self,
        path: &str,
        lhs: NodeId,
        update: &mut dyn FnMut(&str, IrExpr) -> Result<IrExpr, String>,
    ) -> Result<IrStmt, String> {
        let (container, index) = self
            .element_select(lhs)
            .map(|(container, index)| (container.ir, index))
            .ok_or_else(|| {
            format!("read-modify-write of a nested resizable container element in `{path}` is not supported")
        })?;
        if !self.side_effect_free(index) {
            return Err(format!(
                "read-modify-write of a resizable container element in `{path}` requires an index without side effects"
            ));
        }
        let element = self.model.containers[container].element.clone();
        let current = self.lower_expr(path, lhs)?;
        let value = update(path, current)?;
        let value = if element.is_real() {
            if value.is_real() {
                value
            } else {
                IrExpr::new(
                    IrExprKind::CastToReal {
                        a: Box::new(value),
                        shortreal: matches!(element, IrContainerElement::Real { shortreal: true }),
                    },
                    0,
                    true,
                    None,
                )
            }
        } else if let Some((width, signed, two_state)) = element.packed() {
            ir_to_storage(
                apply_assignment_expression_width(value, width),
                width,
                signed,
                two_state,
            )?
        } else {
            return Err(format!(
                "read-modify-write of a resizable container element in `{path}` requires a packed or real element"
            ));
        };
        let operation = match self.model.containers[container].kind {
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => {
                let key = self.lower_string(path, index)?;
                if element.is_real() {
                    IrContainerStmt::SetStringReal {
                        container,
                        key,
                        value,
                    }
                } else {
                    IrContainerStmt::SetString {
                        container,
                        key,
                        value,
                    }
                }
            }
            _ => {
                let index = self.lower_container_top_index(path, container, index)?;
                if element.is_real() {
                    IrContainerStmt::SetReal {
                        container,
                        index,
                        value,
                    }
                } else {
                    IrContainerStmt::Set {
                        container,
                        index,
                        value,
                    }
                }
            }
        };
        Ok(IrStmt::Container(Box::new(operation)))
    }

    /// Whether `node` selects one element of a resizable container with a
    /// single index.
    pub(in super::super) fn is_container_element(&self, node: NodeId) -> bool {
        self.element_select(node).is_some()
    }

    /// Whether evaluating `node` twice is indistinguishable from evaluating
    /// it once: no calls, assignments or increments anywhere inside it.
    pub(in super::super) fn side_effect_free(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } | NodeKind::SysCall { .. } => {
                return false
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::Assignment
                    | Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement,
                ..
            }) => return false,
            _ => {}
        }
        self.node(node)
            .children
            .iter()
            .all(|child| self.side_effect_free(*child))
    }
}
