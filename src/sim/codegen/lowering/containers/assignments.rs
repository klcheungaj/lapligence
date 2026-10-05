//! Assignments.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn lower_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        if let Some(statement) =
            self.lower_stream_container_assignment(path, lhs, rhs, blocking, op)?
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
                return Ok(Some(IrStmt::Container(Box::new(
                    IrContainerStmt::SetContainer {
                        container,
                        indices,
                        source: source.ir,
                    },
                ))));
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
        let selected = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of(*base)
                .map(|container| (container, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of(*base)
                .map(|container| (container, indices[0])),
            _ => None,
        };
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
                return Ok(Some(IrStmt::Container(Box::new(
                    IrContainerStmt::SetContainer {
                        container: container.ir,
                        indices: vec![self.lower_container_index(path, index)?],
                        source: source.ir,
                    },
                ))));
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
        let dst = ContainerInfo { ir: dst };
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
        if let Some(statement) =
            self.lower_bitstream_cast_container_assignment(path, type_node, rhs, &dst)?
        {
            return Ok(statement);
        }
        let descriptor = self.query_descriptor(type_node).cloned();
        if self.assignment_pattern_operands(path, rhs)?.is_some() {
            return self.lower_container_pattern(path, dst.ir, rhs, descriptor.as_ref());
        }
        if let Some(operation) = self.container_method_result(path, dst.ir, rhs)? {
            return Ok(IrStmt::Container(Box::new(operation)));
        }
        let new_array = match self.kind(rhs) {
            NodeKind::Expr(ExprKind::NewArray { size, initializer }) => Some((*size, *initializer)),
            _ => None,
        };
        if let Some((size, initializer)) = new_array {
            if !matches!(self.model.containers[dst.ir].kind, IrContainerKind::Dynamic) {
                return Err(format!("new[] target in `{path}` is not a dynamic array"));
            }
            let size = ir_to_storage(self.lower_expr(path, size)?, 64, true, true)?;
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
        let src = self.container_of(rhs).ok_or_else(|| {
            format!(
                "resizable container assignment in `{path}` requires a compatible array (got {:?})",
                self.kind(rhs)
            )
        })?;
        Ok(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: dst.ir,
            src: src.ir,
        })))
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
        let (container, index) = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of(*base)
                .map(|container| (container.ir, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of(*base)
                .map(|container| (container.ir, indices[0])),
            _ => None,
        }
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
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => self.container_of(*base).is_some(),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                self.container_of(*base).is_some()
            }
            _ => false,
        }
    }

    /// Whether evaluating `node` twice is indistinguishable from evaluating
    /// it once: no calls, assignments or increments anywhere inside it.
    fn side_effect_free(&self, node: NodeId) -> bool {
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
