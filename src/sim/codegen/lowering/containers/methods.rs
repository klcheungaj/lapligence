//! Methods.

use super::*;
use crate::core::db::AggregateLayout;
use crate::sim::ir::{IrBinOp, IrFixedArrayOrderMethod, IrModel, IrPackedSelect};

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
        if matches!(name.as_str(), "sort" | "rsort" | "reverse") {
            if let Some(statement) = self.lower_real_array_order(path, node, receiver, &name)? {
                return Ok(Some(statement));
            }
        }
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
        if name == "shuffle" && !self.db.method_call_has_with_clause(node) {
            // Shuffle a stored packed or real fixed array through a queue copy
            // and store the permutation back in declared order.
            if let Some(copy) = self.fixed_method_copy(receiver)? {
                if !self
                    .container_method_arguments(path, node, receiver)?
                    .is_empty()
                {
                    return Err(format!(
                        "array method `shuffle` in `{path}` takes no arguments"
                    ));
                }
                let mut statements = copy.statements.clone();
                statements.push(IrStmt::Container(Box::new(IrContainerStmt::Method {
                    container: copy.queue,
                    method: IrContainerMethod::Shuffle,
                    callback: None,
                })));
                statements.extend(self.fixed_method_store(&copy));
                return Ok(Some(IrStmt::Block(statements)));
            }
        }
        if self.nested_container_receiver(receiver).is_some() {
            return Err(format!(
                "method `{name}` of a nested container element in `{path}` is not supported"
            ));
        }
        // Statements that build a nested source before the operation.
        let mut prelude = Vec::new();
        let Some(container) = self.container_of(receiver) else {
            return Ok(None);
        };
        let args = self.container_method_arguments(path, node, receiver)?;
        let with_clause = self.db.method_call_has_with_clause(node);
        if matches!(
            self.model.containers[container.ir].element,
            IrContainerElement::Aggregate { .. } | IrContainerElement::FixedArray { .. }
        ) && matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Queue { .. }
        ) {
            // Record elements are built in a typed temporary, then copied in.
            let slot = match (name.as_str(), args.as_slice()) {
                ("push_front", [value]) => Some((crate::sim::ir::IrValueSlot::PushFront, *value)),
                ("push_back", [value]) => Some((crate::sim::ir::IrValueSlot::PushBack, *value)),
                ("insert", [index, value]) => Some((
                    crate::sim::ir::IrValueSlot::Insert(self.lower_queue_method_index_with_end(
                        path,
                        container.ir,
                        *index,
                        true,
                    )?),
                    *value,
                )),
                _ => None,
            };
            if let Some((slot, value)) = slot {
                return self
                    .lower_container_record_push(path, container.ir, slot, value)
                    .map(Some);
            }
        }
        if matches!(name.as_str(), "sort" | "rsort")
            && self.inline_method_needed(
                path,
                node,
                receiver,
                container.ir,
                super::inline_methods::InlineUse::Order,
            )?
        {
            return self
                .lower_inline_order(path, node, receiver, container.ir, name == "rsort", &name)
                .map(Some);
        }
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
                ref element if element.is_handle() => IrContainerStmt::QueuePushFrontChandle {
                    container: container.ir,
                    value: self.lower_container_handle(path, element, *value)?,
                },
                IrContainerElement::Container { .. } => IrContainerStmt::QueuePushFrontContainer {
                    container: container.ir,
                    source: self.nested_container_source(
                        path,
                        container.ir,
                        1,
                        *value,
                        &mut prelude,
                    )?,
                },
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
                ref element if element.is_handle() => IrContainerStmt::QueuePushBackChandle {
                    container: container.ir,
                    value: self.lower_container_handle(path, element, *value)?,
                },
                IrContainerElement::Container { .. } => IrContainerStmt::QueuePushBackContainer {
                    container: container.ir,
                    source: self.nested_container_source(
                        path,
                        container.ir,
                        1,
                        *value,
                        &mut prelude,
                    )?,
                },
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
                    ref element if element.is_handle() => IrContainerStmt::QueueInsertChandle {
                        container: container.ir,
                        index,
                        value: self.lower_container_handle(path, element, *value)?,
                    },
                    IrContainerElement::Container { .. } => IrContainerStmt::QueueInsertContainer {
                        container: container.ir,
                        index,
                        source: self.nested_container_source(
                            path,
                            container.ir,
                            1,
                            *value,
                            &mut prelude,
                        )?,
                    },
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
                    && !matches!(
                        self.model.containers[container.ir].element,
                        IrContainerElement::Real { .. }
                    )
                {
                    return Err(format!(
                        "array method `{name}` in `{path}` currently requires a packed or real dynamic array or queue"
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
                // Ordering methods move whole elements and need no relational
                // operator (SV 7.12.2), so every element type is legal.
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) {
                    return Err(format!(
                        "array method `reverse` in `{path}` requires a dynamic array or queue"
                    ));
                }
                IrContainerStmt::Method {
                    container: container.ir,
                    method: IrContainerMethod::Reverse,
                    callback: None,
                }
            }
            ("shuffle", []) if !with_clause => {
                // Ordering methods move whole elements and need no relational
                // operator (SV 7.12.2), so every element type is legal.
                if !matches!(
                    self.model.containers[container.ir].kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) {
                    return Err(format!(
                        "array method `shuffle` in `{path}` requires a dynamic array or queue"
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
            // `void'(q.pop_front())`: remove the element and discard it
            // (SV 7.10.2.6-7), evaluated once into an unused local.
            ("pop_front" | "pop_back", []) => {
                if matches!(
                    self.model.containers[container.ir].element,
                    IrContainerElement::String
                ) {
                    return Ok(Some(IrStmt::Block(vec![IrStmt::DeclString {
                        name: format!("_llg_pop{}", node.0),
                        init: Some(self.lower_string(path, node)?),
                    }])));
                }
                let Some(value) = self.lower_container_query(path, node)? else {
                    return Ok(None);
                };
                let name = format!("_llg_pop{}", node.0);
                let (width, signed) = (value.width, value.signed);
                return Ok(Some(IrStmt::Block(vec![IrStmt::DeclLocal {
                    name,
                    width,
                    signed,
                    init: Some(Box::new(value)),
                    two_state: false,
                }])));
            }
            _ => return Ok(None),
        };
        let operation = IrStmt::Container(Box::new(operation));
        if prelude.is_empty() {
            return Ok(Some(operation));
        }
        prelude.push(operation);
        Ok(Some(IrStmt::Block(prelude)))
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
        let count = usize::try_from(i64::from(left).abs_diff(i64::from(right)) + 1)
            .map_err(|_| format!("array method `reverse` in `{path}` has too many elements"))?;

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

        if let Some(statement) = self.lower_fixed_array_order_cells(
            path,
            call,
            receiver,
            IrFixedArrayOrderMethod::Reverse,
            &immediate,
            (left, right),
            None,
        )? {
            return Ok(Some(statement));
        }
        let element_width = Self::fixed_descriptor_width(&immediate).ok_or_else(|| {
            format!("array method `reverse` in `{path}` requires a supported fixed element")
        })?;
        let mut statements = Vec::new();
        let mut captured_indices = HashMap::new();
        if let Some(view) =
            self.p30_array_view(path, receiver, &mut statements, &mut captured_indices)?
        {
            let groups =
                fixed_array_permutation_groups(path, &view, count, element_width, "reverse")?;
            let array = self.reference_array(view.array.ir);
            let mut values = Vec::with_capacity(count);
            for cells in &groups {
                let value = fixed_array_group_read(
                    path,
                    array,
                    cells,
                    view.array.elem_width,
                    element_width,
                    immediate.info.signed,
                    element.info.signed,
                )?;
                let name = self.new_fn_name(path, "reverse_value");
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width: element_width,
                    signed: immediate.info.signed,
                    init: Some(Box::new(value)),
                    two_state: immediate.two_state,
                });
                values.push(IrExpr::new(
                    IrExprKind::LocalRead(name),
                    element_width,
                    immediate.info.signed,
                    None,
                ));
            }
            let element_descriptor = element.as_ref();
            for (destination, cells) in groups.iter().enumerate() {
                let rhs = ir_to_storage(
                    values[count - destination - 1].clone(),
                    element_width,
                    immediate.info.signed,
                    immediate.two_state,
                )?;
                statements.extend(fixed_array_group_writes(
                    &self.model,
                    array,
                    cells,
                    view.array.elem_width,
                    element_width,
                    element_descriptor,
                    rhs,
                )?);
            }
            return Ok(Some(IrStmt::Block(statements)));
        }

        let expected_width =
            element_width
                .checked_mul(u32::try_from(count).map_err(|_| {
                    format!("array method `reverse` in `{path}` has too many elements")
                })?)
                .ok_or_else(|| {
                    format!("array method `reverse` in `{path}` exceeds the supported width")
                })?;
        let (source, target) = self.capture_fixed_ordering_receiver(
            path,
            receiver,
            expected_width,
            "reverse",
            &mut statements,
        )?;
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
        let with_node = self.container_method_with_node(path, call, receiver)?;
        if with_node.is_none() && !fixed_sort_integral(&immediate) {
            return Err(format!(
                "array method `{}` in `{path}` requires a fixed integral element",
                if descending { "rsort" } else { "sort" }
            ));
        }
        if with_node.is_some() && Self::fixed_descriptor_width(&immediate).is_none() {
            return Err(format!(
                "array method `{}` in `{path}` requires a supported fixed element for its `with` expression",
                if descending { "rsort" } else { "sort" }
            ));
        }
        if let Some(with_node) = with_node {
            let key_type = self.query_descriptor(with_node).ok_or_else(|| {
                format!(
                    "fixed-array sort with expression in `{path}` has no owned comparison-key type"
                )
            })?;
            if !fixed_sort_integral(key_type) {
                return Err(format!(
                    "fixed-array sort with expression in `{path}` must produce an integral value"
                ));
            }
        }
        let element_width = Self::fixed_descriptor_width(&immediate).ok_or_else(|| {
            format!(
                "array method `{}` in `{path}` requires a supported fixed element",
                if descending { "rsort" } else { "sort" }
            )
        })?;
        let count = usize::try_from(i64::from(left).abs_diff(i64::from(right)) + 1)
            .map_err(|_| format!("fixed-array sort in `{path}` has too many elements"))?;
        if with_node.is_none()
            && !self
                .container_method_arguments(path, call, receiver)?
                .is_empty()
        {
            return Err(format!(
                "fixed-array sort in `{path}` has unexpected arguments"
            ));
        }
        if let Some(statement) = self.lower_fixed_array_order_cells(
            path,
            call,
            receiver,
            if descending {
                IrFixedArrayOrderMethod::RSort
            } else {
                IrFixedArrayOrderMethod::Sort
            },
            &immediate,
            (left, right),
            with_node,
        )? {
            return Ok(Some(statement));
        }
        // Receivers without stored cells (activation values and formals) use
        // a straight-line compare-exchange schedule over a captured value.
        let comparisons = count
            .checked_mul(count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| format!("fixed-array sort in `{path}` has too many comparisons"))?;
        if comparisons > MAX_FIXED_SORT_COMPARISONS {
            return Err(format!(
                "fixed-array sort in `{path}` exceeds the finite comparison schedule limit"
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
            let groups = fixed_array_permutation_groups(
                path,
                &view,
                count,
                element_width,
                if descending { "rsort" } else { "sort" },
            )?;
            let array = self.reference_array(view.array.ir);
            // Each element and key is captured once from its original
            // position, so `item.index` names the element's own index.
            let mut captured = Vec::with_capacity(count);
            for (position, cells) in groups.iter().enumerate() {
                let value = fixed_array_group_read(
                    path,
                    array,
                    cells,
                    view.array.elem_width,
                    element_width,
                    immediate.info.signed,
                    element.info.signed,
                )?;
                captured.push(self.lower_fixed_sort_capture(
                    path,
                    iterator,
                    with_node,
                    &immediate,
                    (left, right),
                    value,
                    sort_index_expr(left, right, position)?,
                    &mut statements,
                )?);
            }
            statements.extend(fixed_sort_schedule(&captured, descending, &immediate)?);
            for (cells, (value, _)) in groups.iter().zip(&captured) {
                statements.extend(fixed_array_group_writes(
                    &self.model,
                    array,
                    cells,
                    view.array.elem_width,
                    element_width,
                    element.as_ref(),
                    value.clone(),
                )?);
            }
            return Ok(Some(IrStmt::Block(statements)));
        }

        let expected_width = element_width
            .checked_mul(
                u32::try_from(count)
                    .map_err(|_| format!("fixed-array sort in `{path}` has too many elements"))?,
            )
            .ok_or_else(|| format!("fixed-array sort in `{path}` exceeds the supported width"))?;
        let (source, target) = self.capture_fixed_ordering_receiver(
            path,
            receiver,
            expected_width,
            if descending { "rsort" } else { "sort" },
            &mut statements,
        )?;
        let mut captured = Vec::with_capacity(count);
        for position in 0..count {
            let offset = sort_offset(position, count, element_width, path)?;
            let value = fixed_reverse_slice(&source, offset, element_width);
            captured.push(self.lower_fixed_sort_capture(
                path,
                iterator,
                with_node,
                &immediate,
                (left, right),
                value,
                sort_index_expr(left, right, position)?,
                &mut statements,
            )?);
        }
        statements.extend(fixed_sort_schedule(&captured, descending, &immediate)?);
        for (position, (value, _)) in captured.iter().enumerate() {
            let lhs = append_fixed_reverse_selection(
                target.clone(),
                sort_offset(position, count, element_width, path)?,
                element_width,
                immediate.info.signed,
                immediate.two_state,
            );
            statements.push(IrStmt::Assign {
                rhs: apply_lhs_assignment_context(&self.model, &lhs, value.clone()),
                lhs,
                nba: false,
            });
        }
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

fn fixed_sort_integral(descriptor: &TypeDescriptor) -> bool {
    matches!(
        descriptor.shape,
        TypeShape::PackedAtom { .. }
            | TypeShape::Aggregate(AggregateLayout {
                kind: AggregateKind::PackedStruct | AggregateKind::PackedUnion,
                ..
            })
    )
}

fn fixed_array_permutation_groups(
    path: &str,
    view: &P30ArrayView,
    count: usize,
    element_width: u32,
    method: &str,
) -> Result<Vec<Vec<Vec<IrExpr>>>, String> {
    if view.array.real
        || view.array.elem_width == 0
        || !element_width.is_multiple_of(view.array.elem_width)
    {
        return Err(format!(
            "array method `{method}` in `{path}` has an unsupported fixed-array representation"
        ));
    }
    let cells_per_element = usize::try_from(element_width / view.array.elem_width)
        .map_err(|_| format!("array method `{method}` in `{path}` has too many row cells"))?;
    if cells_per_element == 0 {
        return Err(format!(
            "array method `{method}` in `{path}` has an empty fixed-array element"
        ));
    }
    let expected_cells = count.checked_mul(cells_per_element).ok_or_else(|| {
        format!("array method `{method}` in `{path}` has too many fixed-array cells")
    })?;
    if view.coordinates.len() != expected_cells {
        return Err(format!(
            "array method `{method}` in `{path}` has an unsupported fixed-array representation"
        ));
    }
    Ok(view
        .coordinates
        .chunks_exact(cells_per_element)
        .map(|cells| cells.to_vec())
        .collect())
}

fn fixed_array_group_read(
    path: &str,
    array: usize,
    cells: &[Vec<IrExpr>],
    cell_width: u32,
    element_width: u32,
    element_signed: bool,
    cell_signed: bool,
) -> Result<IrExpr, String> {
    let parts = cells
        .iter()
        .map(|indices| {
            IrExpr::new(
                IrExprKind::ArrayRead {
                    arr: array,
                    indices: indices.clone(),
                    elem_sel: IrElemSel::Whole,
                },
                cell_width,
                cell_signed,
                None,
            )
        })
        .collect();
    let value = Codegen::join_bitstream_parts(path, parts)?;
    if value.width != element_width {
        return Err(format!(
            "fixed-array element width disagrees with its storage in `{path}`"
        ));
    }
    Ok(IrExpr::resize_to(value, element_width, element_signed))
}

fn fixed_array_group_writes(
    model: &IrModel,
    array: usize,
    cells: &[Vec<IrExpr>],
    cell_width: u32,
    element_width: u32,
    cell_descriptor: &TypeDescriptor,
    value: IrExpr,
) -> Result<Vec<IrStmt>, String> {
    if cell_width == 0 || !element_width.is_multiple_of(cell_width) {
        return Err("fixed-array writeback has incompatible cell and element widths".into());
    }
    let expected_cells = usize::try_from(element_width / cell_width)
        .map_err(|_| "fixed-array writeback has too many row cells")?;
    if cells.len() != expected_cells {
        return Err("fixed-array writeback has an incompatible row shape".into());
    }
    let mut statements = Vec::with_capacity(cells.len());
    for (index, indices) in cells.iter().enumerate() {
        let low_cell = cells.len() - index - 1;
        let offset = u32::try_from(low_cell)
            .ok()
            .and_then(|cell| cell.checked_mul(cell_width))
            .ok_or("fixed-array writeback offset overflows")?;
        let lhs = IrLhs::ArrayElem {
            arr: array,
            indices: indices.clone(),
            elem_sel: IrElemSel::Whole,
        };
        let rhs = fixed_reverse_slice(&value, offset, cell_width);
        let rhs = ir_to_storage(
            rhs,
            cell_width,
            cell_descriptor.info.signed,
            cell_descriptor.two_state,
        )?;
        statements.push(IrStmt::Assign {
            rhs: apply_lhs_assignment_context(model, &lhs, rhs),
            lhs,
            nba: false,
        });
    }
    Ok(statements)
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

/// Straight-line compare-exchange over captured element and key locals. The
/// stored elements are written once afterwards from the final locals.
fn fixed_sort_schedule(
    captured: &[(IrExpr, IrExpr)],
    descending: bool,
    immediate: &TypeDescriptor,
) -> Result<Vec<IrStmt>, String> {
    let local = |expr: &IrExpr| match &expr.kind {
        IrExprKind::LocalRead(name) => Ok(name.clone()),
        _ => Err("fixed-array sort capture is not a local".to_owned()),
    };
    let swap = |first: &IrExpr,
                second: &IrExpr,
                pair: usize,
                two_state: bool|
     -> Result<Vec<IrStmt>, String> {
        let lhs = |expr: &IrExpr| -> Result<IrLhs, String> {
            Ok(IrLhs::WholeRef {
                addr: format!("&{}", local(expr)?),
                width: expr.width,
                signed: expr.signed,
                two_state,
                shortreal: false,
            })
        };
        let saved = format!("{}_swap_{pair}", local(first)?);
        Ok(vec![IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: saved.clone(),
                width: first.width,
                signed: first.signed,
                init: Some(Box::new(first.clone())),
                two_state,
            },
            IrStmt::Assign {
                lhs: lhs(first)?,
                rhs: second.clone(),
                nba: false,
            },
            IrStmt::Assign {
                lhs: lhs(second)?,
                rhs: IrExpr::new(
                    IrExprKind::LocalRead(saved),
                    first.width,
                    first.signed,
                    None,
                ),
                nba: false,
            },
        ])])
    };
    let mut statements = Vec::new();
    for first in 0..captured.len() {
        for second in (first + 1)..captured.len() {
            let (first_value, first_key) = &captured[first];
            let (second_value, second_key) = &captured[second];
            let mut exchange = swap(first_value, second_value, second, immediate.two_state)?;
            if first_key != first_value {
                exchange.extend(swap(first_key, second_key, second, false)?);
            }
            statements.push(IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::Bin {
                        op: if descending { IrBinOp::Lt } else { IrBinOp::Gt },
                        a: Box::new(first_key.clone()),
                        b: Box::new(second_key.clone()),
                    },
                    1,
                    false,
                    None,
                ),
                then_: exchange,
                els: None,
                check: IrUniquePriorityCheck::None,
            });
        }
    }
    Ok(statements)
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
