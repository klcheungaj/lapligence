//! Methods.

use super::*;
use crate::core::db::AggregateLayout;
use crate::sim::ir::IrPackedSelect;

impl<'a> Codegen<'a> {
    pub(in super::super) fn lower_container_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.clone(), *receiver),
            _ => return Ok(None),
        };
        if name == "reverse" {
            if let Some(statement) = self.lower_fixed_array_reverse(path, node, receiver)? {
                return Ok(Some(statement));
            }
        }
        let Some(container) = self.container_of(receiver) else {
            return Ok(None);
        };
        let args = self.container_method_arguments(path, node, receiver)?;
        let with_clause = self.db.method_call_has_with_clause(node);
        let operation = match (name.as_str(), args.as_slice()) {
            ("delete", []) => IrContainerStmt::Delete(container.ir),
            ("delete", [index]) => match self.model.containers[container.ir].kind {
                IrContainerKind::Queue { .. } => IrContainerStmt::DeleteIndex {
                    container: container.ir,
                    index: self.lower_queue_method_index_with_end(
                        path,
                        container.ir,
                        *index,
                        false,
                    )?,
                },
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => IrContainerStmt::DeleteString {
                    container: container.ir,
                    key: self.lower_string(path, *index)?,
                },
                _ => IrContainerStmt::DeleteIndex {
                    container: container.ir,
                    index: self.lower_container_index(path, *index)?,
                },
            },
            ("push_front", [value]) => match self.model.containers[container.ir].element.clone() {
                IrContainerElement::String => IrContainerStmt::QueuePushFrontString {
                    container: container.ir,
                    value: self.lower_string(path, *value)?,
                },
                IrContainerElement::Chandle => IrContainerStmt::QueuePushFrontChandle {
                    container: container.ir,
                    value: self.lower_chandle(path, *value)?,
                },
                IrContainerElement::Container { .. } => {
                    let source = self.container_of(*value).ok_or_else(|| {
                        format!(
                            "recursive queue push_front in {path} requires a dynamic array source"
                        )
                    })?;
                    if !matches!(
                        self.model.containers[source.ir].kind,
                        IrContainerKind::Dynamic
                    ) {
                        return Err(format!(
                            "recursive queue push_front in {path} requires a dynamic array source"
                        ));
                    }
                    IrContainerStmt::QueuePushFrontContainer {
                        container: container.ir,
                        source: source.ir,
                    }
                }
                _ => IrContainerStmt::QueuePushFront {
                    container: container.ir,
                    value: self.lower_container_value(path, container.ir, *value)?,
                },
            },
            ("push_back", [value]) => match self.model.containers[container.ir].element.clone() {
                IrContainerElement::String => IrContainerStmt::QueuePushBackString {
                    container: container.ir,
                    value: self.lower_string(path, *value)?,
                },
                IrContainerElement::Chandle => IrContainerStmt::QueuePushBackChandle {
                    container: container.ir,
                    value: self.lower_chandle(path, *value)?,
                },
                IrContainerElement::Container { .. } => {
                    let source = self.container_of(*value).ok_or_else(|| {
                        format!(
                            "recursive queue push_back in {path} requires a dynamic array source"
                        )
                    })?;
                    if !matches!(
                        self.model.containers[source.ir].kind,
                        IrContainerKind::Dynamic
                    ) {
                        return Err(format!(
                            "recursive queue push_back in {path} requires a dynamic array source"
                        ));
                    }
                    IrContainerStmt::QueuePushBackContainer {
                        container: container.ir,
                        source: source.ir,
                    }
                }
                _ => IrContainerStmt::QueuePushBack {
                    container: container.ir,
                    value: self.lower_container_value(path, container.ir, *value)?,
                },
            },
            ("insert", [index, value]) => {
                let index =
                    self.lower_queue_method_index_with_end(path, container.ir, *index, true)?;
                match self.model.containers[container.ir].element.clone() {
                    IrContainerElement::String => IrContainerStmt::QueueInsertString {
                        container: container.ir,
                        index,
                        value: self.lower_string(path, *value)?,
                    },
                    IrContainerElement::Chandle => IrContainerStmt::QueueInsertChandle {
                        container: container.ir,
                        index,
                        value: self.lower_chandle(path, *value)?,
                    },
                    IrContainerElement::Container { .. } => {
                        let source = self.container_of(*value).ok_or_else(|| {
                            format!(
                                "recursive queue insert in {path} requires a dynamic array source"
                            )
                        })?;
                        if !matches!(
                            self.model.containers[source.ir].kind,
                            IrContainerKind::Dynamic
                        ) {
                            return Err(format!(
                                "recursive queue insert in {path} requires a dynamic array source"
                            ));
                        }
                        IrContainerStmt::QueueInsertContainer {
                            container: container.ir,
                            index,
                            source: source.ir,
                        }
                    }
                    _ => IrContainerStmt::QueueInsert {
                        container: container.ir,
                        index,
                        value: self.lower_container_value(path, container.ir, *value)?,
                    },
                }
            }
            ("sort" | "rsort", []) if !with_clause => {
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !self.model.containers[container.ir].element.is_packed()
                {
                    return Err(format!(
                        "array method `{name}` in `{path}` currently requires a packed dynamic array or queue"
                    ));
                }
                IrContainerStmt::Method {
                    container: container.ir,
                    method: if name == "sort" {
                        IrContainerMethod::Sort
                    } else {
                        IrContainerMethod::RSort
                    },
                    callback: None,
                }
            }
            ("sort" | "rsort", [_with]) if with_clause => {
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !self.model.containers[container.ir].element.is_packed()
                {
                    return Err(format!(
                        "array method `{name}` in `{path}` currently requires a packed dynamic array or queue"
                    ));
                }
                let callback = self
                    .lower_container_method_callback(path, node, receiver, container.ir)?
                    .map(|(callback, _, _, _)| callback);
                IrContainerStmt::Method {
                    container: container.ir,
                    method: if name == "sort" {
                        IrContainerMethod::Sort
                    } else {
                        IrContainerMethod::RSort
                    },
                    callback,
                }
            }
            ("reverse", []) if !with_clause => {
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !self.model.containers[container.ir].element.is_packed()
                {
                    return Err(format!(
                        "array method `reverse` in `{path}` currently requires a packed dynamic array or queue"
                    ));
                }
                IrContainerStmt::Method {
                    container: container.ir,
                    method: IrContainerMethod::Reverse,
                    callback: None,
                }
            }
            ("shuffle", []) if !with_clause => {
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !self.model.containers[container.ir].element.is_packed()
                {
                    return Err(format!(
                        "array method `shuffle` in `{path}` currently requires a packed dynamic array or queue"
                    ));
                }
                IrContainerStmt::Method {
                    container: container.ir,
                    method: IrContainerMethod::Shuffle,
                    callback: None,
                }
            }
            _ if with_clause => {
                return Err(format!(
                    "container method `{name}` with a `with` clause in `{path}` is not supported"
                ));
            }
            _ => return Ok(None),
        };
        Ok(Some(IrStmt::Container(operation)))
    }

    fn lower_fixed_array_reverse(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let Some(descriptor) = self.query_descriptor(receiver).cloned() else {
            return Ok(None);
        };
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Ok(None);
        };
        let Some(&(left, right)) = dimensions.first() else {
            return Err(format!(
                "fixed-array reverse in `{path}` has no declared dimension"
            ));
        };
        let immediate = if dimensions.len() == 1 {
            element.as_ref().clone()
        } else {
            TypeDescriptor {
                shape: TypeShape::FixedArray {
                    dimensions: dimensions[1..].to_vec(),
                    element: element.clone(),
                },
                ..descriptor.clone()
            }
        };
        if !fixed_reverse_integral(&immediate) {
            return Err(format!(
                "array method `reverse` in `{path}` requires a fixed integral element"
            ));
        }
        let element_width = Self::fixed_descriptor_width(&immediate).ok_or_else(|| {
            format!(
                "array method `reverse` in `{path}` requires a supported fixed integral element"
            )
        })?;
        let count = usize::try_from(i64::from(left).abs_diff(i64::from(right)) + 1)
            .map_err(|_| format!("array method `reverse` in `{path}` has too many elements"))?;
        let expected_width =
            element_width
                .checked_mul(u32::try_from(count).map_err(|_| {
                    format!("array method `reverse` in `{path}` has too many elements")
                })?)
                .ok_or_else(|| {
                    format!("array method `reverse` in `{path}` exceeds the supported width")
                })?;

        let with_clause = self.db.method_call_has_with_clause(call);
        if with_clause {
            return Err(format!(
                "fixed-array reverse in `{path}` does not support a `with` clause"
            ));
        }
        let args = self.container_method_arguments(path, call, receiver)?;
        if !args.is_empty() {
            return Err(format!(
                "fixed-array reverse in `{path}` has unexpected arguments"
            ));
        }

        let mut statements = Vec::new();
        let mut captured_indices = HashMap::new();
        if let Some(view) =
            self.p30_array_view(path, receiver, &mut statements, &mut captured_indices)?
        {
            if view.array.real
                || view.array.elem_width != element_width
                || view.coordinates.len() != count
            {
                return Err(format!(
                    "array method `reverse` in `{path}` has an unsupported fixed-array representation"
                ));
            }
            let array = self.reference_array(view.array.ir);
            let two_state = self.model.arrays[array].two_state;
            let mut values = Vec::with_capacity(count);
            for coordinates in &view.coordinates {
                let value = IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: array,
                        indices: coordinates.clone(),
                        elem_sel: IrElemSel::Whole,
                    },
                    element_width,
                    immediate.info.signed,
                    None,
                );
                let name = self.new_fn_name(path, "reverse_value");
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width: element_width,
                    signed: immediate.info.signed,
                    init: Some(Box::new(value)),
                    two_state,
                });
                values.push(IrExpr::new(
                    IrExprKind::LocalRead(name),
                    element_width,
                    immediate.info.signed,
                    None,
                ));
            }
            for (destination, coordinates) in view.coordinates.iter().enumerate() {
                let rhs = ir_to_storage(
                    values[count - destination - 1].clone(),
                    element_width,
                    immediate.info.signed,
                    immediate.two_state,
                )?;
                let lhs = IrLhs::ArrayElem {
                    arr: array,
                    indices: coordinates.clone(),
                    elem_sel: IrElemSel::Whole,
                };
                statements.push(IrStmt::Assign {
                    lhs: lhs.clone(),
                    rhs: apply_lhs_assignment_context(&self.model, &lhs, rhs),
                    nba: false,
                });
            }
            return Ok(Some(IrStmt::Block(statements)));
        }

        let source = self
            .fixed_activation_read(path, receiver)?
            .ok_or_else(|| format!("fixed-array reverse in `{path}` has no readable storage"))?;
        if source.is_real() || source.width != expected_width {
            return Err(format!(
                "array method `reverse` in `{path}` has an unsupported fixed-array representation"
            ));
        }
        let target = self
            .fixed_activation_lhs(path, receiver)?
            .ok_or_else(|| format!("fixed-array reverse in `{path}` has no writable storage"))?;
        let name = self.new_fn_name(path, "reverse_source");
        statements.push(IrStmt::DeclLocal {
            name: name.clone(),
            width: source.width,
            signed: source.signed,
            init: Some(Box::new(source)),
            two_state: descriptor.two_state,
        });
        let source = IrExpr::new(
            IrExprKind::LocalRead(name),
            expected_width,
            descriptor.info.signed,
            None,
        );
        for destination in 0..count {
            let source_offset = u32::try_from(destination)
                .ok()
                .and_then(|index| index.checked_mul(element_width))
                .ok_or_else(|| format!("array method `reverse` in `{path}` offset overflows"))?;
            let destination_offset = u32::try_from(count - destination - 1)
                .ok()
                .and_then(|index| index.checked_mul(element_width))
                .ok_or_else(|| format!("array method `reverse` in `{path}` offset overflows"))?;
            let rhs = fixed_reverse_slice(&source, source_offset, element_width);
            let lhs = append_fixed_reverse_selection(
                target.clone(),
                destination_offset,
                element_width,
                immediate.info.signed,
                immediate.two_state,
            );
            let rhs = ir_to_storage(
                rhs,
                element_width,
                immediate.info.signed,
                immediate.two_state,
            )?;
            statements.push(IrStmt::Assign {
                lhs: lhs.clone(),
                rhs: apply_lhs_assignment_context(&self.model, &lhs, rhs),
                nba: false,
            });
        }
        Ok(Some(IrStmt::Block(statements)))
    }
}

fn fixed_reverse_integral(descriptor: &TypeDescriptor) -> bool {
    matches!(
        descriptor.shape,
        TypeShape::PackedAtom { .. }
            | TypeShape::Aggregate(AggregateLayout {
                kind: AggregateKind::PackedStruct | AggregateKind::PackedUnion,
                ..
            })
    )
}

fn fixed_reverse_slice(value: &IrExpr, offset: u32, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::IdxPartSel {
            base: Box::new(value.clone()),
            base_idx: Box::new(lhs_integer_expr(i128::from(offset))),
            width_expr: Box::new(lhs_integer_expr(i128::from(width))),
            neg: false,
        },
        width,
        false,
        None,
    )
}

fn append_fixed_reverse_selection(
    target: IrLhs,
    offset: u32,
    width: u32,
    signed: bool,
    two_state: bool,
) -> IrLhs {
    let step = IrPackedSelect {
        base: lhs_integer_expr(i128::from(offset)),
        width,
    };
    match target {
        IrLhs::PackedSelect {
            target, mut steps, ..
        } => {
            steps.push(step);
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            }
        }
        target => IrLhs::PackedSelect {
            target: Box::new(target),
            steps: vec![step],
            signed,
            two_state,
        },
    }
}
