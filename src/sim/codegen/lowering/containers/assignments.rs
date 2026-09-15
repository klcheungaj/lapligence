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
        if self.p30_fixed_array_assignment_candidate(lhs) {
            return self.lower_p30_fixed_array_assignment(path, lhs, rhs, blocking, op);
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
                return Ok(Some(IrStmt::Container(IrContainerStmt::SetContainer {
                    container,
                    indices,
                    source: source.ir,
                })));
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
                    IrContainerElement::Chandle => IrContainerStmt::SetNestedChandle {
                        container,
                        indices,
                        value: self.lower_chandle(path, rhs)?,
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
                return Ok(Some(IrStmt::Container(operation)));
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
                return Err(format!(
                    "compound assignment to resizable container element in `{path}` is not supported"
                ));
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
                return Ok(Some(IrStmt::Container(IrContainerStmt::SetContainer {
                    container: container.ir,
                    indices: vec![self.lower_container_index(path, index)?],
                    source: source.ir,
                })));
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
                        IrContainerElement::Chandle => IrContainerStmt::SetStringChandle {
                            container: container.ir,
                            key,
                            value: self.lower_chandle(path, rhs)?,
                        },
                        _ => {
                            return Err(format!(
                                "string-keyed associative element write in `{path}` requires a scalar value"
                            ))
                        }
                    }
                }
                _ => {
                    let index = if matches!(
                        self.model.containers[container.ir].kind,
                        IrContainerKind::Queue { .. }
                    ) {
                        self.lower_queue_index(path, container.ir, index)?
                    } else {
                        self.lower_container_index(path, index)?
                    };
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
                        IrContainerElement::Chandle => IrContainerStmt::SetChandleValue {
                            container: container.ir,
                            index,
                            value: self.lower_chandle(path, rhs)?,
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
            return Ok(Some(IrStmt::Container(operation)));
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
        if let Some(statement) =
            self.lower_bitstream_cast_container_assignment(path, lhs, rhs, &dst)?
        {
            return Ok(Some(statement));
        }
        let descriptor = self.query_descriptor(lhs).cloned();
        if let NodeKind::Expr(ExprKind::Operation { op: pattern_op, .. }) = self.kind(rhs) {
            if *pattern_op == Operation::AssignmentPattern {
                return Ok(Some(self.lower_container_pattern(
                    path,
                    dst.ir,
                    rhs,
                    descriptor.as_ref(),
                )?));
            }
        }
        if let Some(operation) = self.container_method_result(path, dst.ir, rhs)? {
            return Ok(Some(IrStmt::Container(operation)));
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
            return Ok(Some(IrStmt::Container(IrContainerStmt::DynamicNew {
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
                return Ok(Some(IrStmt::Container(IrContainerStmt::QueueAssign {
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
        Ok(Some(IrStmt::Container(IrContainerStmt::Copy {
            dst: dst.ir,
            src: src.ir,
        })))
    }
}
