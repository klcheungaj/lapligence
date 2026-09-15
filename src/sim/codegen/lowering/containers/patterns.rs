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
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(pattern)
        else {
            return Err(format!(
                "resizable container initializer in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "resizable container initializer in `{path}` is not an assignment pattern"
            ));
        }
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
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
            TypeShape::Container { element, .. } => Some(element.as_ref().clone()),
            _ => None,
        });
        let element_two_state = self.model.containers[container].element.two_state();
        let mut explicit = Vec::<(i64, NodeId)>::new();
        let mut type_values = Vec::<(AssignmentPatternKeyType, NodeId)>::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
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
            if let Some(index) = parse_pattern_i128(key) {
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
            let Some(element_descriptor) = element_descriptor.as_ref() else {
                return Err(format!(
                    "resizable container assignment pattern type key `{key}` has no captured element type in `{path}`"
                ));
            };
            if !super::super::collection::pattern_key_matches_type_descriptor(
                key_type,
                element_descriptor,
                element_two_state,
            ) {
                return Err(format!(
                    "resizable container assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            }
            if type_values
                .iter()
                .any(|(previous, _)| super::super::collection::pattern_key_types_equal(previous, key_type))
            {
                return Err(format!(
                    "duplicate resizable container assignment pattern type key `{key}` in `{path}`"
                ));
            }
            type_values.push((key_type.clone(), value));
        }
        let Some(max_index) = explicit.iter().map(|(index, _)| *index).max() else {
            return Err(format!(
                "resizable container assignment pattern needs an explicit size or index in `{path}`"
            ));
        };
        let count = usize::try_from(max_index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or_else(|| {
                format!("resizable container assignment pattern is too large in `{path}`")
            })?;
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            let index = i64::try_from(index).map_err(|_| {
                format!("resizable container assignment pattern is too large in `{path}`")
            })?;
            let value = explicit
                .iter()
                .find(|(key, _)| *key == index)
                .map(|(_, value)| *value)
                .or_else(|| type_values.last().map(|(_, value)| *value))
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
        if matches!(
            &element,
            IrContainerElement::String | IrContainerElement::Chandle
        ) {
            return self.lower_associative_object_pattern(path, container, operands, element);
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
                        IrContainerElement::Real { .. } => IrContainerStmt::SetStringReal {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: lowered,
                        },
                        _ => IrContainerStmt::SetString {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: lowered,
                        },
                    }));
                }
                IrContainerKind::Associative { key: assoc_key, .. } => {
                    let index = parse_pattern_i128(key_text).ok_or_else(|| {
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
                        IrContainerElement::Real { .. } => IrContainerStmt::SetReal {
                            container,
                            index: pattern_key_expr(index, width, signed, two_state),
                            value: lowered,
                        },
                        _ => IrContainerStmt::Set {
                            container,
                            index: pattern_key_expr(index, width, signed, two_state),
                            value: lowered,
                        },
                    }));
                }
                _ => unreachable!(),
            }
        }
        // Assignment patterns replace an associative array.  Evaluate all
        // element expressions into temporaries before clearing the destination
        // so side effects and overlapping source reads retain one evaluation.
        captures.push(IrStmt::Container(IrContainerStmt::Delete(container)));
        captures.push(IrStmt::Container(IrContainerStmt::ResetDefault(container)));
        if let Some(value) = default_value {
            captures.push(IrStmt::Container(IrContainerStmt::SetDefault {
                container,
                value,
            }));
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
                        IrContainerElement::Chandle => IrContainerStmt::SetStringChandle {
                            container,
                            key: IrStringExpr::Literal(bytes),
                            value: self.lower_chandle(path, value)?,
                        },
                        _ => unreachable!(),
                    }
                }
                IrContainerKind::Associative { key: assoc_key, .. } => {
                    let index = parse_pattern_i128(key).ok_or_else(|| {
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
                        IrContainerElement::Chandle => IrContainerStmt::SetChandleValue {
                            container,
                            index,
                            value: self.lower_chandle(path, value)?,
                        },
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            };
            writes.push(IrStmt::Container(operation));
        }
        let mut captures = vec![
            IrStmt::Container(IrContainerStmt::Delete(container)),
            IrStmt::Container(IrContainerStmt::ResetDefault(container)),
        ];
        if let Some(value) = default {
            captures.push(IrStmt::Container(match &element {
                IrContainerElement::String => IrContainerStmt::SetDefaultString {
                    container,
                    value: self.lower_string(path, value)?,
                },
                IrContainerElement::Chandle => IrContainerStmt::SetDefaultChandle {
                    container,
                    value: self.lower_chandle(path, value)?,
                },
                _ => unreachable!(),
            }));
        }
        captures.extend(writes);
        Ok(IrStmt::Block(captures))
    }

    fn lower_container_source_values(
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
                captures.push(IrStmt::Container(IrContainerStmt::AssignRealValues {
                    container,
                    values: rewritten,
                }));
                Ok(IrStmt::Block(captures))
            }
            IrContainerElement::String => {
                let values = source_values
                    .into_iter()
                    .map(|value| self.lower_string(path, value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::Container(IrContainerStmt::AssignStringValues {
                    container,
                    values,
                }))
            }
            IrContainerElement::Chandle => {
                let values = source_values
                    .into_iter()
                    .map(|value| self.lower_chandle(path, value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStmt::Container(IrContainerStmt::AssignChandleValues {
                    container,
                    values,
                }))
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
        captures.push(IrStmt::Container(IrContainerStmt::AssignValues {
            container,
            values: rewritten,
        }));
        Ok(IrStmt::Block(captures))
    }
}
