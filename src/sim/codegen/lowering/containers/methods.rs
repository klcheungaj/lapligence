//! Methods.

use super::*;
use crate::core::db::AggregateLayout;
use crate::sim::ir::{IrBinOp, IrPackedSelect};

const MAX_FIXED_SORT_COMPARISONS: usize = 1 << 20;

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
        if matches!(name.as_str(), "sort" | "rsort") {
            if let Some(statement) =
                self.lower_fixed_array_sort(path, node, receiver, name == "rsort")?
            {
                return Ok(Some(statement));
            }
        }
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

    fn lower_fixed_array_sort(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        descending: bool,
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
                "fixed-array sort in `{path}` has no declared dimension"
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
        if !fixed_sort_integral(&immediate) {
            return Err(format!(
                "array method `{}` in `{path}` requires a fixed integral element",
                if descending { "rsort" } else { "sort" }
            ));
        }
        let element_width = Self::fixed_descriptor_width(&immediate).ok_or_else(|| {
            format!(
                "array method `{}` in `{path}` requires a supported fixed integral element",
                if descending { "rsort" } else { "sort" }
            )
        })?;
        let count = usize::try_from(i64::from(left).abs_diff(i64::from(right)) + 1)
            .map_err(|_| format!("fixed-array sort in `{path}` has too many elements"))?;
        let comparisons = count
            .checked_mul(count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| format!("fixed-array sort in `{path}` has too many comparisons"))?;
        if comparisons > MAX_FIXED_SORT_COMPARISONS {
            return Err(format!(
                "fixed-array sort in `{path}` exceeds the finite comparison schedule limit"
            ));
        }
        let expected_width = element_width
            .checked_mul(
                u32::try_from(count)
                    .map_err(|_| format!("fixed-array sort in `{path}` has too many elements"))?,
            )
            .ok_or_else(|| format!("fixed-array sort in `{path}` exceeds the supported width"))?;

        let with_node = self.container_method_with_node(path, call, receiver)?;
        if with_node.is_none()
            && !self
                .container_method_arguments(path, call, receiver)?
                .is_empty()
        {
            return Err(format!(
                "fixed-array sort in `{path}` has unexpected arguments"
            ));
        }
        let iterator = if with_node.is_some() {
            Some(self.db.method_call_iterator(call).ok_or_else(|| {
                format!("fixed-array sort in `{path}` has no captured iterator binding")
            })?)
        } else {
            None
        };

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
                    "array method `{}` in `{path}` has an unsupported fixed-array representation",
                    if descending { "rsort" } else { "sort" }
                ));
            }
            let array = self.reference_array(view.array.ir);
            let initial_statement_count = statements.len();
            for first in 0..count {
                for second in (first + 1)..count {
                    let mut pair = Vec::new();
                    let first_value = IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: array,
                            indices: view.coordinates[first].clone(),
                            elem_sel: IrElemSel::Whole,
                        },
                        element_width,
                        immediate.info.signed,
                        None,
                    );
                    let second_value = IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: array,
                            indices: view.coordinates[second].clone(),
                            elem_sel: IrElemSel::Whole,
                        },
                        element_width,
                        immediate.info.signed,
                        None,
                    );
                    let first_index = sort_index_expr(left, right, first)?;
                    let second_index = sort_index_expr(left, right, second)?;
                    let (first_value, first_key) = self.lower_fixed_sort_capture(
                        path,
                        iterator,
                        with_node,
                        &immediate,
                        (left, right),
                        first_value,
                        first_index,
                        &mut pair,
                    )?;
                    let (second_value, second_key) = self.lower_fixed_sort_capture(
                        path,
                        iterator,
                        with_node,
                        &immediate,
                        (left, right),
                        second_value,
                        second_index,
                        &mut pair,
                    )?;
                    let first_lhs = IrLhs::ArrayElem {
                        arr: array,
                        indices: view.coordinates[first].clone(),
                        elem_sel: IrElemSel::Whole,
                    };
                    let second_lhs = IrLhs::ArrayElem {
                        arr: array,
                        indices: view.coordinates[second].clone(),
                        elem_sel: IrElemSel::Whole,
                    };
                    pair.push(fixed_sort_swap(
                        descending,
                        first_value,
                        second_value,
                        first_key,
                        second_key,
                        first_lhs,
                        second_lhs,
                    ));
                    statements.push(IrStmt::Block(pair));
                }
            }
            debug_assert_eq!(comparisons, statements.len() - initial_statement_count);
            return Ok(Some(IrStmt::Block(statements)));
        }

        let source = self
            .fixed_activation_read(path, receiver)?
            .ok_or_else(|| format!("fixed-array sort in `{path}` has no readable storage"))?;
        if source.is_real() || source.width != expected_width {
            return Err(format!(
                "array method `{}` in `{path}` has an unsupported fixed-array representation",
                if descending { "rsort" } else { "sort" }
            ));
        }
        let target = self
            .fixed_activation_lhs(path, receiver)?
            .ok_or_else(|| format!("fixed-array sort in `{path}` has no writable storage"))?;
        for first in 0..count {
            for second in (first + 1)..count {
                let mut pair = Vec::new();
                let first_offset = sort_offset(first, count, element_width, path)?;
                let second_offset = sort_offset(second, count, element_width, path)?;
                let source = self.fixed_activation_read(path, receiver)?.ok_or_else(|| {
                    format!("fixed-array sort in `{path}` has no readable storage")
                })?;
                let first_value = fixed_reverse_slice(&source, first_offset, element_width);
                let second_value = fixed_reverse_slice(&source, second_offset, element_width);
                let first_index = sort_index_expr(left, right, first)?;
                let second_index = sort_index_expr(left, right, second)?;
                let (first_value, first_key) = self.lower_fixed_sort_capture(
                    path,
                    iterator,
                    with_node,
                    &immediate,
                    (left, right),
                    first_value,
                    first_index,
                    &mut pair,
                )?;
                let (second_value, second_key) = self.lower_fixed_sort_capture(
                    path,
                    iterator,
                    with_node,
                    &immediate,
                    (left, right),
                    second_value,
                    second_index,
                    &mut pair,
                )?;
                let first_lhs = append_fixed_reverse_selection(
                    target.clone(),
                    first_offset,
                    element_width,
                    immediate.info.signed,
                    immediate.two_state,
                );
                let second_lhs = append_fixed_reverse_selection(
                    target.clone(),
                    second_offset,
                    element_width,
                    immediate.info.signed,
                    immediate.two_state,
                );
                pair.push(fixed_sort_swap(
                    descending,
                    first_value,
                    second_value,
                    first_key,
                    second_key,
                    first_lhs,
                    second_lhs,
                ));
                statements.push(IrStmt::Block(pair));
            }
        }
        debug_assert_eq!(comparisons, statements.len());
        Ok(Some(IrStmt::Block(statements)))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_fixed_sort_capture(
        &mut self,
        path: &str,
        iterator: Option<NodeId>,
        with_node: Option<NodeId>,
        immediate: &TypeDescriptor,
        bounds: (i32, i32),
        value: IrExpr,
        index: IrExpr,
        statements: &mut Vec<IrStmt>,
    ) -> Result<(IrExpr, IrExpr), String> {
        let item_name = self.new_fn_name(path, "sort_item");
        let item = ir_to_storage(
            value,
            Self::fixed_descriptor_width(immediate)
                .ok_or_else(|| format!("fixed-array sort in `{path}` has no element width"))?,
            immediate.info.signed,
            immediate.two_state,
        )?;
        let item_width = item.width;
        statements.push(IrStmt::DeclLocal {
            name: item_name.clone(),
            width: item_width,
            signed: immediate.info.signed,
            init: Some(Box::new(item)),
            two_state: immediate.two_state,
        });
        let item = IrExpr::new(
            IrExprKind::LocalRead(item_name.clone()),
            item_width,
            immediate.info.signed,
            None,
        );
        let Some(with_node) = with_node else {
            return Ok((item.clone(), item));
        };
        let iterator = iterator.ok_or_else(|| {
            format!("fixed-array sort in `{path}` has no captured iterator binding")
        })?;
        let index_name = self.new_fn_name(path, "sort_index");
        statements.push(IrStmt::DeclLocal {
            name: index_name.clone(),
            width: index.width,
            signed: index.signed,
            init: Some(Box::new(index)),
            two_state: true,
        });
        let saved = self.fixed_method_iterators.insert(
            iterator,
            FixedMethodIterator {
                descriptor: immediate.clone(),
                dimensions: vec![bounds],
                index_names: vec![index_name],
                item_name: item_name.clone(),
            },
        );
        let mapped = self.lower_expr(path, with_node);
        if let Some(saved) = saved {
            self.fixed_method_iterators.insert(iterator, saved);
        } else {
            self.fixed_method_iterators.remove(&iterator);
        }
        let mapped = mapped?;
        if mapped.is_real() || mapped.width == 0 {
            return Err(format!(
                "fixed-array sort with expression in `{path}` must produce an integral value"
            ));
        }
        let mapped_width = mapped.width;
        let mapped_signed = mapped.signed;
        let mapped_two_state = self.db.is_two_state_type(with_node);
        let mapped = ir_to_storage(mapped, mapped_width, mapped_signed, mapped_two_state)?;
        let key_name = self.new_fn_name(path, "sort_key");
        let key_width = mapped.width;
        let key_signed = mapped.signed;
        statements.push(IrStmt::DeclLocal {
            name: key_name.clone(),
            width: key_width,
            signed: key_signed,
            init: Some(Box::new(mapped)),
            two_state: mapped_two_state,
        });
        Ok((
            item,
            IrExpr::new(IrExprKind::LocalRead(key_name), key_width, key_signed, None),
        ))
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

fn fixed_sort_integral(descriptor: &TypeDescriptor) -> bool {
    fixed_reverse_integral(descriptor)
}

fn sort_index_expr(left: i32, right: i32, offset: usize) -> Result<IrExpr, String> {
    let offset =
        i32::try_from(offset).map_err(|_| "fixed-array sort index offset overflows".to_owned())?;
    let index = if left >= right {
        left.checked_sub(offset)
    } else {
        left.checked_add(offset)
    }
    .ok_or_else(|| "fixed-array sort index overflows".to_owned())?;
    Ok(pattern_key_expr(i128::from(index), 32, true, false))
}

fn sort_offset(offset: usize, count: usize, width: u32, path: &str) -> Result<u32, String> {
    let physical = count
        .checked_sub(offset + 1)
        .ok_or_else(|| format!("fixed-array sort offset overflows in `{path}`"))?;
    u32::try_from(physical)
        .ok()
        .and_then(|offset| offset.checked_mul(width))
        .ok_or_else(|| format!("fixed-array sort offset overflows in `{path}`"))
}

fn fixed_sort_swap(
    descending: bool,
    first_value: IrExpr,
    second_value: IrExpr,
    first_key: IrExpr,
    second_key: IrExpr,
    first_lhs: IrLhs,
    second_lhs: IrLhs,
) -> IrStmt {
    let condition = IrExpr::new(
        IrExprKind::Bin {
            op: if descending { IrBinOp::Lt } else { IrBinOp::Gt },
            a: Box::new(first_key),
            b: Box::new(second_key),
        },
        1,
        false,
        None,
    );
    IrStmt::If {
        cond: condition,
        then_: vec![
            IrStmt::Assign {
                lhs: first_lhs,
                rhs: second_value,
                nba: false,
            },
            IrStmt::Assign {
                lhs: second_lhs,
                rhs: first_value,
                nba: false,
            },
        ],
        els: None,
        check: IrUniquePriorityCheck::None,
    }
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
