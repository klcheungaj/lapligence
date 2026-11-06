//! Patterns.

use super::*;

impl<'a> Codegen<'a> {
    pub(super) fn lower_container_pattern(
        &mut self,
        path: &str,
        container: usize,
        pattern: NodeId,
        descriptor: Option<&TypeDescriptor>,
    ) -> Result<IrStmt, String> {
        let Some(operands) = self.assignment_pattern_operands(path, pattern)? else {
            return Err(format!(
                "resizable container initializer in `{path}` is not an assignment pattern"
            ));
        };
        if let Some(dimensions) = self.fixed_view_dims.get(&container).cloned() {
            // A multidimensional view: its nested pattern lists the cells in
            // declaration (row-major) order.
            let element = descriptor.and_then(|descriptor| match &descriptor.shape {
                TypeShape::FixedArray { element, .. } => Some(element.as_ref().clone()),
                _ => None,
            });
            let mut cells = Vec::new();
            self.flatten_multidimensional_pattern(
                path,
                &dimensions,
                operands,
                element.as_ref(),
                &mut cells,
            )?;
            return self.lower_container_source_values(path, container, cells);
        }
        let kind = self.model.containers[container].kind.clone();
        match kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                self.lower_sequence_pattern(path, container, operands, descriptor)
            }
            IrContainerKind::Associative { .. } => {
                self.lower_associative_pattern(path, container, operands)
            }
        }
    }

    /// The cells of a pattern over `dimensions`, one nested pattern per
    /// dimension, in row-major order (IEEE 1800-2009 10.9.1). A level is
    /// positional (one item per index) or keyed: index keys name declared
    /// indices of that dimension, and a `default:` or type-key value that
    /// is not itself a pattern fills every cell below the index it covers.
    fn flatten_multidimensional_pattern(
        &mut self,
        path: &str,
        dimensions: &[(i32, i32)],
        operands: Vec<NodeId>,
        element: Option<&TypeDescriptor>,
        cells: &mut Vec<NodeId>,
    ) -> Result<(), String> {
        let unsupported = || {
            format!(
                "a multidimensional fixed array in `{path}` takes only nested assignment patterns whose items are elements or patterns"
            )
        };
        let Some(((left, right), inner)) = dimensions.split_first() else {
            return Err(unsupported());
        };
        let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        // Each index of this dimension with its item and whether the item
        // fills every cell below it (a default or type-key value).
        let mut items = Vec::new();
        if tagged {
            let mut explicit = Vec::<(i128, NodeId)>::new();
            let mut typed = None;
            let mut default = None;
            for operand in operands {
                let NodeKind::Expr(ExprKind::TaggedPattern {
                    key,
                    key_type,
                    value,
                    ..
                }) = self.kind(operand)
                else {
                    return Err(format!(
                        "mixed positional and keyed assignment pattern in `{path}` is not supported"
                    ));
                };
                let (key, key_type, value) = (key.clone(), key_type.clone(), *value);
                let key = key.ok_or_else(unsupported)?;
                let value = value.ok_or_else(unsupported)?;
                if key == "default" {
                    default = Some(value);
                } else if let Some(index) = self.assignment_pattern_index_key(path, operand)? {
                    let (left, right) = (i128::from(*left), i128::from(*right));
                    if index < left.min(right) || index > left.max(right) {
                        return Err(format!(
                            "fixed-array assignment pattern index `{key}` is outside the declared range in `{path}`"
                        ));
                    }
                    let offset = if left <= right {
                        index - left
                    } else {
                        left - index
                    };
                    if explicit.iter().any(|(previous, _)| *previous == offset) {
                        return Err(format!(
                            "duplicate fixed-array assignment pattern index `{key}` in `{path}`"
                        ));
                    }
                    explicit.push((offset, value));
                } else if let Some(key_type) = key_type {
                    let matches = element.is_some_and(|element| {
                        super::super::collection::pattern_key_matches_type_descriptor(
                            &key_type,
                            element,
                            element.two_state,
                        )
                    });
                    if matches {
                        typed = Some(value);
                    }
                } else {
                    return Err(unsupported());
                }
            }
            for offset in 0..i128::from(extent) {
                let item = explicit
                    .iter()
                    .find(|(index, _)| *index == offset)
                    .map(|(_, value)| (*value, false))
                    .or(typed.map(|value| (value, true)))
                    .or(default.map(|value| (value, true)))
                    .ok_or_else(|| {
                        format!(
                            "fixed-array assignment pattern does not cover every index in `{path}`"
                        )
                    })?;
                items.push(item);
            }
        } else {
            if u64::try_from(operands.len()).ok() != Some(extent) {
                return Err(unsupported());
            }
            items.extend(operands.into_iter().map(|operand| (operand, false)));
        }
        if inner.is_empty() {
            cells.extend(items.into_iter().map(|(value, _)| value));
            return Ok(());
        }
        let below = inner
            .iter()
            .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
            .product::<u64>();
        for (value, fills) in items {
            match self.assignment_pattern_operands(path, value)? {
                Some(row) => {
                    self.flatten_multidimensional_pattern(path, inner, row, element, cells)?
                }
                None if fills => {
                    let below = usize::try_from(below).map_err(|_| unsupported())?;
                    cells.extend(std::iter::repeat_n(value, below));
                }
                None => return Err(unsupported()),
            }
        }
        Ok(())
    }

    fn lower_sequence_pattern(
        &mut self,
        path: &str,
        container: usize,
        operands: Vec<NodeId>,
        descriptor: Option<&TypeDescriptor>,
    ) -> Result<IrStmt, String> {
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        if !tagged {
            if matches!(
                self.model.containers[container].element,
                IrContainerElement::Container { key: Some(_), .. }
            ) {
                return self.lower_associative_elements_pattern(path, container, operands);
            }
            return self.lower_container_source_values(path, container, operands);
        }
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            return Err(format!(
                "mixed positional and keyed resizable-container assignment pattern in `{path}` is not supported"
            ));
        }

        let element_descriptor = descriptor.and_then(|descriptor| match &descriptor.shape {
            TypeShape::Container { element, .. } | TypeShape::FixedArray { element, .. } => {
                Some(element.as_ref().clone())
            }
            _ => None,
        });
        // A fixed-array view has its declared size and HDL index range; keys
        // name declared indices (SV 10.9.1).
        let fixed_view = self.model.containers[container]
            .initial_size
            .zip(self.fixed_view_ranges.get(&container).copied());
        let element_two_state = self.model.containers[container].element.two_state();
        let mut explicit = Vec::<(i64, NodeId)>::new();
        let mut type_values = Vec::<(AssignmentPatternKeyType, NodeId)>::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
                ..
            }) = self.kind(operand)
            else {
                continue;
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("resizable container assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!(
                    "resizable container assignment pattern key `{key}` has no value in `{path}`"
                )
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in resizable container assignment pattern in `{path}`"
                    ));
                }
                continue;
            }
            if let Some(index) = self.assignment_pattern_index_key(path, operand)? {
                let index = match fixed_view {
                    Some((_, (left, right))) => {
                        let (left, right) = (i128::from(left), i128::from(right));
                        if index < left.min(right) || index > left.max(right) {
                            return Err(format!(
                                "fixed-array assignment pattern index `{key}` is outside the declared range in `{path}`"
                            ));
                        }
                        if left <= right {
                            index - left
                        } else {
                            left - index
                        }
                    }
                    None => index,
                };
                let index = i64::try_from(index).map_err(|_| {
                    format!(
                        "resizable container assignment pattern index `{key}` is out of bounds in `{path}`"
                    )
                })?;
                if index < 0 {
                    return Err(format!(
                        "resizable container assignment pattern index `{key}` is out of bounds in `{path}`"
                    ));
                }
                if explicit.iter().any(|(previous, _)| *previous == index) {
                    return Err(format!(
                        "duplicate resizable container assignment pattern index `{key}` in `{path}`"
                    ));
                }
                explicit.push((index, value));
                continue;
            }
            let Some(key_type) = key_type else {
                return Err(format!(
                    "resizable container assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            };
            type_values.push(((**key_type).clone(), value));
        }
        let count = match fixed_view {
            Some((size, _)) => usize::try_from(size)
                .map_err(|_| format!("fixed-array assignment pattern is too large in `{path}`"))?,
            None => {
                let Some(max_index) = explicit.iter().map(|(index, _)| *index).max() else {
                    return Err(format!(
                        "resizable container assignment pattern needs an explicit size or index in `{path}`"
                    ));
                };
                usize::try_from(max_index)
                    .ok()
                    .and_then(|index| index.checked_add(1))
                    .ok_or_else(|| {
                        format!("resizable container assignment pattern is too large in `{path}`")
                    })?
            }
        };
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            let index = i64::try_from(index).map_err(|_| {
                format!("resizable container assignment pattern is too large in `{path}`")
            })?;
            let value = explicit
                .iter()
                .find(|(key, _)| *key == index)
                .map(|(_, value)| *value)
                .or_else(|| {
                    element_descriptor.as_ref().and_then(|element_descriptor| {
                        type_values.iter().rev().find_map(|(key_type, value)| {
                            super::super::collection::pattern_key_matches_type_descriptor(
                                key_type,
                                element_descriptor,
                                element_two_state,
                            )
                            .then_some(*value)
                        })
                    })
                })
                .or(default)
                .ok_or_else(|| {
                    format!(
                        "resizable container assignment pattern does not cover index `{index}` in `{path}`"
                    )
                })?;
            values.push(value);
        }
        self.lower_container_source_values(path, container, values)
    }

    /// A lexical container of the type nested in `container`'s element. A
    /// queue is built as a dynamic array: element transfer copies either.
    fn nested_pattern_temporary(&mut self, element: &IrContainerElement) -> usize {
        let (key, inner) = match element {
            IrContainerElement::Container { key, element, .. } => (key.clone(), *element.clone()),
            other => (None, other.clone()),
        };
        let temporary = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{temporary}"),
            element: inner,
            kind: key.map_or(IrContainerKind::Dynamic, |key| {
                IrContainerKind::Associative { key }
            }),
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        temporary
    }

    /// `aa = '{"x": '{1: 2}}`: every element is built in place in a fresh
    /// container, so sources reading `container` see its old elements.
    fn lower_associative_nested_pattern(
        &mut self,
        path: &str,
        container: usize,
        operands: Vec<NodeId>,
        element: IrContainerElement,
    ) -> Result<IrStmt, String> {
        let string_key = matches!(
            self.model.containers[container].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        );
        let building = self.container_temporary_like(container);
        let mut statements = vec![IrStmt::Container(Box::new(IrContainerStmt::Declare(
            building,
        )))];
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key: Some(key),
                key_type: None,
                value: Some(value),
                ..
            }) = self.kind(operand)
            else {
                return Err(format!(
                    "a pattern key of nested associative array in `{path}` is not a literal key"
                ));
            };
            let (key, value) = (key.clone(), *value);
            if key == "default" {
                return Err(format!(
                    "a `default` key for a nested container element in `{path}` is not supported"
                ));
            }
            let root = if string_key {
                let bytes = parse_pattern_string_key(&key).ok_or_else(|| {
                    format!(
                        "string associative assignment pattern key `{key}` is not a literal in `{path}`"
                    )
                })?;
                IrChandleExpr::ContainerElement {
                    container: building,
                    indices: Vec::new(),
                    key: Some(Box::new(IrStringExpr::Literal(bytes))),
                    write: true,
                }
            } else {
                let index = self
                    .assignment_pattern_index_key(path, operand)?
                    .ok_or_else(|| {
                        format!(
                            "integral associative assignment pattern key `{key}` is not a constant in `{path}`"
                        )
                    })?;
                IrChandleExpr::ContainerElement {
                    container: building,
                    indices: vec![pattern_key_expr(index, 64, true, false)],
                    key: None,
                    write: true,
                }
            };
            let root = crate::sim::ir::IrValueItemRoot::Element(root);
            let temporary = self.nested_pattern_temporary(&element);
            let associative = matches!(
                self.model.containers[temporary].kind,
                IrContainerKind::Associative { .. }
            );
            statements.push(IrStmt::Container(Box::new(if associative {
                IrContainerStmt::BindElement {
                    root: root.clone(),
                    container: temporary,
                }
            } else {
                IrContainerStmt::Declare(temporary)
            })));
            statements.push(self.lower_container_into(path, value, temporary, value)?);
            if !associative {
                statements.push(IrStmt::Container(Box::new(
                    IrContainerStmt::ContainerToValueItem {
                        container: temporary,
                        root,
                        items: Vec::new(),
                    },
                )));
            }
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: container,
            src: building,
        })));
        Ok(IrStmt::Block(statements))
    }

    /// `qa = '{'{7: 1}, '{8: 2}}`: associative elements of a queue or dynamic
    /// array, built in a fresh container like other element patterns.
    fn lower_associative_elements_pattern(
        &mut self,
        path: &str,
        container: usize,
        values: Vec<NodeId>,
    ) -> Result<IrStmt, String> {
        let element = self.model.containers[container].element.clone();
        let queue = matches!(
            self.model.containers[container].kind,
            IrContainerKind::Queue { .. }
        );
        let building = self.container_temporary_like(container);
        let mut statements = vec![IrStmt::Container(Box::new(IrContainerStmt::Declare(
            building,
        )))];
        if !queue {
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::DynamicNew {
                container: building,
                size: pattern_key_expr(values.len() as i128, 64, false, true),
                initializer: None,
            })));
        }
        for (position, value) in values.into_iter().enumerate() {
            let temporary = self.nested_pattern_temporary(&element);
            if queue {
                statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                    temporary,
                ))));
            } else {
                statements.push(IrStmt::Container(Box::new(IrContainerStmt::BindElement {
                    root: crate::sim::ir::IrValueItemRoot::Element(
                        IrChandleExpr::ContainerElement {
                            container: building,
                            indices: vec![pattern_key_expr(position as i128, 64, false, true)],
                            key: None,
                            write: true,
                        },
                    ),
                    container: temporary,
                })));
            }
            statements.push(self.lower_container_into(path, value, temporary, value)?);
            if queue {
                statements.push(IrStmt::Container(Box::new(
                    IrContainerStmt::QueuePushBackContainer {
                        container: building,
                        source: temporary,
                    },
                )));
            }
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: container,
            src: building,
        })));
        Ok(IrStmt::Block(statements))
    }

    fn lower_associative_pattern(
        &mut self,
        path: &str,
        container: usize,
        operands: Vec<NodeId>,
    ) -> Result<IrStmt, String> {
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            return Err(format!(
                "positional assignment pattern for associative array in `{path}` is not supported"
            ));
        }
        let kind = self.model.containers[container].kind.clone();
        let element = self.model.containers[container].element.clone();
        if element.is_string() || element.is_handle() {
            return self.lower_associative_object_pattern(path, container, operands, element);
        }
        if let IrContainerElement::Container { .. } = element {
            return self.lower_associative_nested_pattern(path, container, operands, element);
        }
        let mut seen_integral = Vec::<i128>::new();
        let mut seen_string = Vec::<Vec<u8>>::new();
        let mut captures = Vec::new();
        let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
        let mut default_value = None;
        let mut writes = Vec::new();
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
                ..
            }) = self.kind(operand)
            else {
                continue;
            };
            let key_text = key.as_deref().ok_or_else(|| {
                format!("associative assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("associative assignment pattern key `{key_text}` has no value in `{path}`")
            })?;
            if key_text == "default" {
                if key_type.is_some() {
                    return Err(format!(
                        "associative assignment pattern default key in `{path}` has an unexpected type"
                    ));
                }
                if default_value.is_some() {
                    return Err(format!(
                        "duplicate default key for associative array assignment pattern in `{path}`"
                    ));
                }
                let lowered = if let Some((name, width, signed)) = captured.get(&value) {
                    IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
                } else {
                    let lowered = self.lower_container_value(path, container, value)?;
                    let name = format!("_assoc{}_{}", container, value.0);
                    let (width, signed) = (lowered.width, lowered.signed);
                    captures.push(IrStmt::DeclLocal {
                        name: name.clone(),
                        width,
                        signed,
                        two_state: false,
                        init: Some(Box::new(lowered)),
                    });
                    captured.insert(value, (name.clone(), width, signed));
                    IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
                };
                default_value = Some(lowered);
                continue;
            }
            if key_type.is_some() {
                return Err(format!(
                    "type key for associative array assignment pattern `{key_text}` is not supported"
                ));
            }
            let lowered = if let Some((name, width, signed)) = captured.get(&value) {
                IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
            } else {
                let lowered = self.lower_container_value(path, container, value)?;
                let name = format!("_assoc{}_{}", container, value.0);
                let (width, signed) = (lowered.width, lowered.signed);
                captures.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(lowered)),
                });
                captured.insert(value, (name.clone(), width, signed));
                IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
            };
            match &kind {
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => {
                    let bytes = parse_pattern_string_key(key_text).ok_or_else(|| {
                        format!(
                            "string associative assignment pattern key `{key_text}` is not a literal in `{path}`"
                        )
                    })?;
                    if seen_string.iter().any(|previous| previous == &bytes) {
                        return Err(format!(
                            "duplicate associative assignment pattern key `{key_text}` in `{path}`"
                        ));
                    }
                    seen_string.push(bytes.clone());
                    writes.push(IrStmt::Container(match &element {
                        IrContainerElement::Real { .. } => {
                            Box::new(IrContainerStmt::SetStringReal {
                                container,
                                key: IrStringExpr::Literal(bytes),
                                value: lowered,
                            })
                        }
                        _ => Box::new(IrContainerStmt::SetString {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: lowered,
                        }),
                    }));
                }
                IrContainerKind::Associative { key: assoc_key, .. } => {
                    let index = self
                        .assignment_pattern_index_key(path, operand)?
                        .ok_or_else(|| {
                        format!(
                            "integral associative assignment pattern key `{key_text}` is not a constant in `{path}`"
                        )
                    })?;
                    if seen_integral.contains(&index) {
                        return Err(format!(
                            "duplicate associative assignment pattern key `{key_text}` in `{path}`"
                        ));
                    }
                    seen_integral.push(index);
                    let (width, signed, two_state) = match assoc_key {
                        IrAssocKey::Integral {
                            width,
                            signed,
                            two_state,
                        } => (*width, *signed, *two_state),
                        IrAssocKey::Wildcard => (32, true, false),
                        IrAssocKey::String => unreachable!(),
                    };
                    writes.push(IrStmt::Container(match &element {
                        IrContainerElement::Real { .. } => Box::new(IrContainerStmt::SetReal {
                            container,
                            index: pattern_key_expr(index, width, signed, two_state),
                            value: lowered,
                        }),
                        _ => Box::new(IrContainerStmt::Set {
                            container,
                            index: pattern_key_expr(index, width, signed, two_state),
                            value: lowered,
                        }),
                    }));
                }
                _ => unreachable!(),
            }
        }
        // Assignment patterns replace an associative array.  Evaluate all
        // element expressions into temporaries before clearing the destination
        // so side effects and overlapping source reads retain one evaluation.
        captures.push(IrStmt::Container(Box::new(IrContainerStmt::Delete(
            container,
        ))));
        captures.push(IrStmt::Container(Box::new(IrContainerStmt::ResetDefault(
            container,
        ))));
        if let Some(value) = default_value {
            captures.push(IrStmt::Container(Box::new(IrContainerStmt::SetDefault {
                container,
                value,
            })));
        }
        captures.extend(writes);
        Ok(IrStmt::Block(captures))
    }

    fn lower_associative_object_pattern(
        &mut self,
        path: &str,
        container: usize,
        operands: Vec<NodeId>,
        element: IrContainerElement,
    ) -> Result<IrStmt, String> {
        let kind = self.model.containers[container].kind.clone();
        let mut seen_integral = Vec::<i128>::new();
        let mut seen_string = Vec::<Vec<u8>>::new();
        let mut writes = Vec::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
                ..
            }) = self.kind(operand)
            else {
                continue;
            };
            if key_type.is_some() {
                return Err(format!(
                    "type key for associative array assignment pattern is not supported in `{path}`"
                ));
            }
            let key = key.as_deref().ok_or_else(|| {
                format!("associative assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("associative assignment pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.is_some() {
                    return Err(format!(
                        "duplicate default key for associative array assignment pattern in `{path}`"
                    ));
                }
                default = Some(value);
                continue;
            }
            let operation = match &kind {
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => {
                    let bytes = parse_pattern_string_key(key).ok_or_else(|| {
                        format!("string associative assignment pattern key `{key}` is not a literal in `{path}`")
                    })?;
                    if seen_string.iter().any(|previous| previous == &bytes) {
                        return Err(format!(
                            "duplicate associative assignment pattern key `{key}` in `{path}`"
                        ));
                    }
                    seen_string.push(bytes.clone());
                    match &element {
                        IrContainerElement::String => IrContainerStmt::SetStringString {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: self.lower_string(path, value)?,
                        },
                        element if element.is_handle() => IrContainerStmt::SetStringChandle {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: self.lower_container_handle(path, element, value)?,
                        },
                        _ => unreachable!(),
                    }
                }
                IrContainerKind::Associative { key: assoc_key, .. } => {
                    let index = self
                        .assignment_pattern_index_key(path, operand)?
                        .ok_or_else(|| {
                            format!("integral associative assignment pattern key `{key}` is not a constant in `{path}`")
                        })?;
                    if seen_integral.contains(&index) {
                        return Err(format!(
                            "duplicate associative assignment pattern key `{key}` in `{path}`"
                        ));
                    }
                    seen_integral.push(index);
                    let (width, signed, two_state) = match assoc_key {
                        IrAssocKey::Integral {
                            width,
                            signed,
                            two_state,
                        } => (*width, *signed, *two_state),
                        IrAssocKey::Wildcard => (32, true, false),
                        IrAssocKey::String => unreachable!(),
                    };
                    let index = pattern_key_expr(index, width, signed, two_state);
                    match &element {
                        IrContainerElement::String => IrContainerStmt::SetStringValue {
                            container,
                            index,
                            value: self.lower_string(path, value)?,
                        },
                        element if element.is_handle() => IrContainerStmt::SetChandleValue {
                            container,
                            index,
                            value: self.lower_container_handle(path, element, value)?,
                        },
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            };
            writes.push(IrStmt::Container(Box::new(operation)));
        }
        let mut captures = vec![
            IrStmt::Container(Box::new(IrContainerStmt::Delete(container))),
            IrStmt::Container(Box::new(IrContainerStmt::ResetDefault(container))),
        ];
        if let Some(value) = default {
            captures.push(IrStmt::Container(match &element {
                IrContainerElement::String => Box::new(IrContainerStmt::SetDefaultString {
                    container,
                    value: self.lower_string(path, value)?,
                }),
                element if element.is_handle() => Box::new(IrContainerStmt::SetDefaultChandle {
                    container,
                    value: self.lower_container_handle(path, element, value)?,
                }),
                _ => unreachable!(),
            }));
        }
        captures.extend(writes);
        Ok(IrStmt::Block(captures))
    }

    pub(super) fn lower_container_source_values(
        &mut self,
        path: &str,
        container: usize,
        source_values: Vec<NodeId>,
    ) -> Result<IrStmt, String> {
        match self.model.containers[container].element.clone() {
            IrContainerElement::Packed { .. } => {
                self.lower_container_source_packed_values(path, container, source_values)
            }
            IrContainerElement::Real { .. } => {
                let mut captures = Vec::new();
                let mut captured = HashMap::<NodeId, String>::new();
                let mut rewritten = Vec::with_capacity(source_values.len());
                for value in source_values {
                    let expression = if let Some(name) = captured.get(&value) {
                        IrExpr::new(IrExprKind::LocalRead(name.clone()), 0, false, None)
                    } else {
                        let expression = self.lower_expr(path, value)?;
                        let name = format!("_container{}_{}", container, value.0);
                        captures.push(IrStmt::DeclLocal {
                            name: name.clone(),
                            width: 0,
                            signed: false,
                            two_state: false,
                            init: Some(Box::new(expression)),
                        });
                        captured.insert(value, name.clone());
                        IrExpr::new(IrExprKind::LocalRead(name), 0, false, None)
                    };
                    rewritten.push(expression);
                }
                captures.push(IrStmt::Container(Box::new(IrContainerStmt::AssignRealValues {
                    container,
                    values: rewritten,
                })));
                Ok(IrStmt::Block(captures))
            }
            IrContainerElement::String => {
                let values = source_values
                    .into_iter()
                    .map(|value| self.lower_string(path, value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::Container(Box::new(IrContainerStmt::AssignStringValues {
                    container,
                    values,
                })))
            }
            element if element.is_handle() => {
                let values = source_values
                    .into_iter()
                    .map(|value| self.lower_container_handle(path, &element, value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::Container(Box::new(IrContainerStmt::AssignChandleValues {
                    container,
                    values,
                })))
            }
            IrContainerElement::Aggregate { .. }
            | IrContainerElement::Union { .. }
            | IrContainerElement::FixedArray { .. } => {
                self.lower_container_record_values(path, container, source_values)
            }
            _ => Err(format!(
                "resizable container assignment pattern in `{path}` requires a directly represented scalar element"
            )),
        }
    }

    fn lower_container_source_packed_values(
        &mut self,
        path: &str,
        container: usize,
        source_values: Vec<NodeId>,
    ) -> Result<IrStmt, String> {
        let mut captures = Vec::new();
        let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
        let mut rewritten = Vec::with_capacity(source_values.len());
        for value in source_values {
            let lowered = if let Some((name, width, signed)) = captured.get(&value) {
                IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
            } else {
                let lowered = self.lower_container_value(path, container, value)?;
                let name = format!("_container{}_{}", container, value.0);
                let (width, signed) = (lowered.width, lowered.signed);
                captures.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(lowered)),
                });
                captured.insert(value, (name.clone(), width, signed));
                IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
            };
            rewritten.push(lowered);
        }
        captures.push(IrStmt::Container(Box::new(IrContainerStmt::AssignValues {
            container,
            values: rewritten,
        })));
        Ok(IrStmt::Block(captures))
    }

    /// Source of a nested container element write (`q.push_back(src)`,
    /// `c[i] = src`): a dynamic-array variable passes directly; a pattern or
    /// an unpacked concatenation is first built in a fresh dynamic-array
    /// temporary declared by `prelude`.
    pub(in super::super) fn nested_container_source(
        &mut self,
        path: &str,
        container: usize,
        depth: usize,
        value: NodeId,
        prelude: &mut Vec<IrStmt>,
    ) -> Result<usize, String> {
        let source_variable = self.container_of(self.p30_unwrap_cast(value));
        let nested_key = match self.container_element_type(container, depth) {
            Some(IrContainerElement::Container { key: Some(key), .. }) => Some(key),
            _ => None,
        };
        if let (Some(_), Some(source)) = (&nested_key, &source_variable) {
            return Ok(source.ir);
        }
        if let Some(source) = &source_variable {
            match self.model.containers[source.ir].kind {
                IrContainerKind::Dynamic => return Ok(source.ir),
                // A queue is copied into a dynamic temporary below (SV 7.6).
                IrContainerKind::Queue { .. } => {}
                IrContainerKind::Associative { .. } => {
                    return Err(format!(
                        "nested container write in `{path}` from an associative variable is not supported; use a dynamic array, queue, pattern or concatenation"
                    ))
                }
            }
        }
        let Some(IrContainerElement::Container { element, .. }) =
            self.container_element_type(container, depth)
        else {
            return Err(format!(
                "nested container write in `{path}` does not select a container element"
            ));
        };
        let temporary = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{temporary}"),
            element: *element,
            kind: match nested_key {
                Some(key) => IrContainerKind::Associative { key },
                None => IrContainerKind::Dynamic,
            },
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
            temporary,
        ))));
        if let Some(source) = source_variable {
            prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
                dst: temporary,
                src: source.ir,
            })));
            return Ok(temporary);
        }
        let source = self.p30_unwrap_cast(value);
        let concat = match self.kind(source) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) if operands
                .iter()
                .all(|operand| self.container_of(*operand).is_none()) =>
            {
                Some(operands.clone())
            }
            _ => None,
        };
        prelude.push(match concat {
            // `{a, b}` lists the new elements in order (SV 10.10).
            Some(operands) => self.lower_container_source_values(path, temporary, operands)?,
            None => self.lower_container_into(path, value, temporary, value)?,
        });
        Ok(temporary)
    }
}
