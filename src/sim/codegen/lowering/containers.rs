//! Lowering for resizable unpacked containers.

use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrContainerElement, IrContainerMethod, IrContainerReduction, IrObjectType,
    IrQueueBound, IrQueueSource, IrStringExpr,
};

/// A fixed-array view represented by complete coordinates in logical
/// (declared left-to-right) order.
#[derive(Clone)]
struct P30ArrayView {
    array: ArrayInfo,
    coordinates: Vec<Vec<IrExpr>>,
}

impl<'a> Codegen<'a> {
    pub(super) fn emit_array_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.array_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (array, initializer)) in initializers.into_iter().enumerate() {
            let inst = self.owning_inst(array).ok_or_else(|| {
                format!(
                    "fixed-array initializer for `{}` has no owning instance",
                    self.node(array).name
                )
            })?;
            self.inst = inst;
            let path = self.instance_path_of(inst);
            let body = self
                .lower_p30_fixed_array_assignment(
                    &path,
                    array,
                    initializer,
                    true,
                    Operation::Assignment,
                )?
                .ok_or_else(|| {
                    format!(
                        "fixed-array initializer for `{}` in `{path}` has no array target",
                        self.node(array).name
                    )
                })?;
            processes.push(IrProcess::new_with_origin(
                self.new_fn_name(&path, "array_init"),
                format!("{path}.array_initializer.{index}"),
                IrShape::RunOnce,
                Vec::new(),
                vec![body],
                self.origin(array),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    pub(super) fn emit_container_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.container_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (owner, container)) in initializers.into_iter().enumerate() {
            let initializer = self
                .db
                .array_meta(owner)
                .and_then(|meta| meta.initializer())
                .ok_or_else(|| {
                    format!(
                        "container initializer for `{}` disappeared before lowering",
                        self.node(owner).full_name()
                    )
                })?;
            let descriptor = self.db.type_descriptor(owner).cloned();
            let path = self.node(owner).full_name();
            let body =
                self.lower_container_pattern(path, container, initializer, descriptor.as_ref())?;
            let name = format!(
                "p_{}_container_init_{index}",
                ident(&self.model.design_name)
            );
            let label = format!("{}.container_initializer.{index}", self.model.design_name);
            processes.push(IrProcess::new_with_origin(
                name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![body],
                self.origin(owner),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    fn lower_container_pattern(
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
            if !super::collection::pattern_key_matches_type_descriptor(
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
                .any(|(previous, _)| super::collection::pattern_key_types_equal(previous, key_type))
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

    fn container_method_arguments(
        &self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<Vec<NodeId>, String> {
        let mut arguments = self.node(call).children.clone();
        let receiver_index = arguments
            .iter()
            .position(|child| *child == receiver)
            .ok_or_else(|| format!("container method in `{path}` has no receiver child"))?;
        arguments.remove(receiver_index);
        Ok(arguments)
    }

    /// Return the structural child carrying a method's `with` expression.
    /// Slang visits that expression before adding the receiver edge, so this
    /// intentionally removes the receiver by identity instead of relying on
    /// child order.
    fn container_method_with_node(
        &self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<Option<NodeId>, String> {
        if !self.db.method_call_has_with_clause(call) {
            return Ok(None);
        }
        let mut arguments = self.container_method_arguments(path, call, receiver)?;
        if arguments.len() != 1 {
            return Err(format!(
                "container method in `{path}` has an invalid with-clause argument list"
            ));
        }
        Ok(arguments.pop())
    }

    /// Lower one packed iterator expression into a callback understood by the
    /// C container runtime. The callback uses the frontend-captured iterator
    /// declaration identity, never a source spelling guessed from `item`.
    fn lower_container_method_callback(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        container: usize,
    ) -> Result<Option<(String, u32, bool, bool)>, String> {
        let Some(with_node) = self.container_method_with_node(path, call, receiver)? else {
            return Ok(None);
        };
        if self.event_context(with_node)?.is_some() {
            return Err(format!(
                "container method with-clause in `{path}` cannot capture automatic locals or formals"
            ));
        }
        let Some(iterator) = self.db.method_call_iterator(call) else {
            return Err(format!(
                "container method with-clause in `{path}` has no iterator binding"
            ));
        };
        let Some((source_width, source_signed, _source_two_state)) =
            self.model.containers[container].element.packed()
        else {
            return Err(format!(
                "container method with-clause in `{path}` requires a packed element type"
            ));
        };
        let (index_width, index_signed) = match self.model.containers[container].kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => (32, true),
            IrContainerKind::Associative {
                key:
                    IrAssocKey::Integral {
                        width,
                        signed,
                        two_state: _,
                    },
            } => (width, signed),
            IrContainerKind::Associative {
                key: IrAssocKey::String | IrAssocKey::Wildcard,
            } => (0, false),
        };
        let saved_iterator = self.container_iterator.replace(ContainerIterator {
            node: iterator,
            item_width: source_width,
            item_signed: source_signed,
            index_width,
            index_signed,
        });
        let value = self.lower_expr(path, with_node);
        self.container_iterator = saved_iterator;
        let value = value?;
        let result_width = value.width;
        let result_signed = value.signed;
        let result_two_state = self.db.is_two_state_type(with_node);
        if value.is_real() || value.width == 0 {
            return Err(format!(
                "container method with-clause in `{path}` must produce an integral value"
            ));
        }
        let callback = self.new_fn_name(path, "container_eval");
        self.pending_container_pre_fns
            .push(crate::sim::ir::IrPreFn::MonEval {
                c_name: callback.clone(),
                args: vec![value],
                context: None,
                item: true,
            });
        Ok(Some((
            callback,
            result_width,
            result_signed,
            result_two_state,
        )))
    }

    fn container_method_result(
        &mut self,
        path: &str,
        dst: usize,
        rhs: NodeId,
    ) -> Result<Option<IrContainerStmt>, String> {
        let rhs = self.p30_unwrap_cast(rhs);
        let (name, receiver) = match self.kind(rhs) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => (name.clone(), *receiver),
            _ => return Ok(None),
        };
        let Some(source) = self.container_of(receiver) else {
            return Ok(None);
        };
        let method = match name.as_str() {
            "find" => IrContainerMethod::Find,
            "find_index" => IrContainerMethod::FindIndex,
            "find_first" => IrContainerMethod::FindFirst,
            "find_first_index" => IrContainerMethod::FindFirstIndex,
            "find_last" => IrContainerMethod::FindLast,
            "find_last_index" => IrContainerMethod::FindLastIndex,
            "min" => IrContainerMethod::Min,
            "max" => IrContainerMethod::Max,
            "unique" => IrContainerMethod::Unique,
            "unique_index" => IrContainerMethod::UniqueIndex,
            _ => return Ok(None),
        };
        if !matches!(
            self.model.containers[dst].kind,
            IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "array method `{name}` in `{path}` returns a queue and requires a queue destination"
            ));
        }
        if !self.model.containers[source.ir].element.is_packed()
            || !self.model.containers[dst].element.is_packed()
        {
            return Err(format!(
                "array method `{name}` in `{path}` currently requires packed source and destination elements"
            ));
        }
        if matches!(
            self.model.containers[source.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::Wildcard | IrAssocKey::String
            }
        ) && matches!(
            method,
            IrContainerMethod::FindIndex
                | IrContainerMethod::FindFirstIndex
                | IrContainerMethod::FindLastIndex
                | IrContainerMethod::UniqueIndex
        ) {
            return Err(format!(
                "array method `{name}` in `{path}` requires an integral-key associative array for packed index results"
            ));
        }
        let callback = self
            .lower_container_method_callback(path, rhs, receiver, source.ir)?
            .map(|(callback, _, _, _)| callback);
        if matches!(
            method,
            IrContainerMethod::Find
                | IrContainerMethod::FindIndex
                | IrContainerMethod::FindFirst
                | IrContainerMethod::FindFirstIndex
                | IrContainerMethod::FindLast
                | IrContainerMethod::FindLastIndex
        ) && callback.is_none()
        {
            return Err(format!(
                "array locator method `{name}` in `{path}` requires a with clause"
            ));
        }
        Ok(Some(IrContainerStmt::MethodAssign {
            dst,
            src: source.ir,
            method,
            callback,
        }))
    }

    fn lower_container_value(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        let element = self.model.containers[container].element.clone();
        self.lower_container_value_for_element(path, &element, node)
    }

    fn lower_container_value_for_element(
        &mut self,
        path: &str,
        element: &IrContainerElement,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if element.is_real() {
            let value = self.lower_expr(path, node)?;
            return if value.is_real() {
                Ok(value)
            } else {
                Ok(IrExpr::new(
                    IrExprKind::CastToReal {
                        a: Box::new(value),
                        shortreal: matches!(element, IrContainerElement::Real { shortreal: true }),
                    },
                    0,
                    true,
                    None,
                ))
            };
        }
        let Some((width, signed, two_state)) = element.packed() else {
            return Err(format!("container element type is not packed in {path}"));
        };
        let value = self.lower_expr(path, node)?;
        let value = apply_assignment_expression_width(value, width);
        ir_to_storage(value, width, signed, two_state)
    }

    fn lower_queue_method_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, 32, true, true)
    }

    fn is_unbounded_node(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Unbounded) => true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.is_unbounded_node(*target),
            _ => false,
        }
    }

    fn lower_queue_method_index_with_end(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
        allow_end: bool,
    ) -> Result<IrExpr, String> {
        if self.is_unbounded_node(node) {
            let size = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Size(container))),
                32,
                true,
                None,
            );
            if allow_end {
                return Ok(size);
            }
            let one = pattern_key_expr(1, 32, true, false);
            return Ok(IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Sub,
                    a: Box::new(size),
                    b: Box::new(one),
                },
                32,
                true,
                None,
            ));
        }
        self.lower_queue_method_index(path, node)
    }

    pub(super) fn lower_queue_index(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        self.lower_queue_method_index_with_end(path, container, node, false)
    }

    fn lower_queue_bound(&mut self, path: &str, node: NodeId) -> Result<IrQueueBound, String> {
        if self.is_unbounded_node(node) {
            return Ok(IrQueueBound::Unbounded);
        }
        Ok(IrQueueBound::Value(self.lower_container_index(path, node)?))
    }

    fn lower_queue_sources(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<IrQueueSource>>, String> {
        let node = self.p30_unwrap_cast(node);
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                reordered,
                ..
            }) => {
                let mut sources = Vec::new();
                for operand in operands {
                    let Some(mut nested) = self.lower_queue_sources(path, *operand)? else {
                        return Ok(None);
                    };
                    sources.append(&mut nested);
                }
                if *reordered {
                    sources.reverse();
                }
                Ok(Some(sources))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let Some(source) = self.container_of(*base) else {
                    return Ok(None);
                };
                if !matches!(
                    self.model.containers[source.ir].kind,
                    IrContainerKind::Queue { .. }
                ) {
                    return Ok(None);
                }
                Ok(Some(vec![IrQueueSource::Slice {
                    container: source.ir,
                    left: self.lower_queue_bound(path, *left)?,
                    right: self.lower_queue_bound(path, *right)?,
                }]))
            }
            _ => Ok(self
                .container_of(node)
                .filter(|source| {
                    matches!(
                        self.model.containers[source.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|source| vec![IrQueueSource::Whole(source.ir)])),
        }
    }

    fn lower_container_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        if value.is_real() {
            return Err(format!(
                "resizable container index in `{path}` must be integral"
            ));
        }
        Ok(value)
    }

    fn container_key_address(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(String, Option<usize>, u32, bool, bool), String> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .last()
                .copied()
                .flatten()
                .or_else(|| refs.first().copied().flatten()),
            _ => Some(node),
        };
        if let Some(target) = target {
            if let Some(signal) = self.sig_globals.get(&target) {
                return Ok((
                    format!("&{}", signal.global),
                    Some(signal.ir),
                    signal.width,
                    signal.signed,
                    signal.two_state,
                ));
            }
            if let Some(local) = self.proc_local_info(target) {
                return Ok((
                    format!("&{}", local.c_name),
                    local.static_signal.as_ref().map(|signal| signal.ir),
                    local.width,
                    local.signed,
                    local.two_state,
                ));
            }
            if let Some(function) = &self.func {
                if let Some((name, width, signed, two_state, _shortreal)) =
                    function.locals.get(&target)
                {
                    return Ok((format!("&{name}"), None, *width, *signed, *two_state));
                }
            }
        }
        let source_name = &self.node(node).name;
        if let Some(signal) = self
            .scope_sig_names
            .get(path)
            .and_then(|names| names.get(source_name))
        {
            return Ok((
                format!("&{}", signal.global),
                Some(signal.ir),
                signal.width,
                signal.signed,
                signal.two_state,
            ));
        }
        if let Some(signal) = self
            .scope_sig_names
            .values()
            .find_map(|names| names.get(source_name))
        {
            return Ok((
                format!("&{}", signal.global),
                Some(signal.ir),
                signal.width,
                signal.signed,
                signal.two_state,
            ));
        }
        let lhs = self.lower_lhs(path, node).map_err(|error| {
            format!(
                "{error}; associative traversal key `{source_name}` resolved target is {:?}",
                target.map(|target| self.kind(target))
            )
        })?;
        match lhs {
            IrLhs::Whole(index) => {
                let ty = self.model.signals[index].ty;
                Ok((
                    format!("&{}", self.model.signals[index].c_name),
                    Some(index),
                    ty.width(),
                    ty.signed(),
                    ty.two_state(),
                ))
            }
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                ..
            } => Ok((addr, None, width, signed, two_state)),
            _ => Err(format!(
                "associative traversal key in `{path}` must be a whole packed variable"
            )),
        }
    }

    pub(super) fn container_of(&self, node: NodeId) -> Option<ContainerInfo> {
        match self.kind(node) {
            NodeKind::Array { .. } => self.container_globals.get(&node).cloned(),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.container_globals.get(target).cloned(),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .first()
                .copied()
                .flatten()
                .or_else(|| refs.last().copied().flatten())
                .and_then(|target| self.container_globals.get(&target).cloned()),
            _ => None,
        }
    }

    fn container_element_path(&self, node: NodeId) -> Option<(usize, Vec<NodeId>)> {
        let (base, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if !indices.is_empty() => {
                (*base, indices.clone())
            }
            _ => return None,
        };
        if let Some((container, mut prefix)) = self.container_element_path(base) {
            prefix.extend(indices);
            return Some((container, prefix));
        }
        if let Some((container, key)) = self.associative_integral_element(base) {
            let mut prefix = vec![key];
            prefix.extend(indices);
            return Some((container, prefix));
        }
        let container = self.container_of(base)?;
        match self.model.containers[container.ir].kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                Some((container.ir, indices))
            }
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
            } => Some((container.ir, indices)),
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => None,
        }
    }

    fn associative_integral_element(&self, node: NodeId) -> Option<(usize, NodeId)> {
        let (base, key) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            _ => return None,
        };
        let container = self.container_of(base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard
            }
        )
        .then_some((container.ir, key))
    }

    fn associative_string_element(&self, node: NodeId) -> Option<(usize, NodeId)> {
        let (base, key) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            _ => return None,
        };
        let container = self.container_of(base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        )
        .then_some((container.ir, key))
    }

    fn container_element_type(&self, container: usize, depth: usize) -> Option<IrContainerElement> {
        let mut element = self.model.containers[container].element.clone();
        for _ in 1..depth {
            let IrContainerElement::Container { element: next, .. } = element else {
                return None;
            };
            element = *next;
        }
        Some(element)
    }

    fn lower_container_path_indices(
        &mut self,
        path: &str,
        container: usize,
        indices: Vec<NodeId>,
    ) -> Result<Vec<IrExpr>, String> {
        indices
            .into_iter()
            .enumerate()
            .map(|(depth, index)| {
                if depth == 0
                    && matches!(
                        self.model.containers[container].kind,
                        IrContainerKind::Queue { .. }
                    )
                {
                    self.lower_queue_index(path, container, index)
                } else {
                    self.lower_container_index(path, index)
                }
            })
            .collect()
    }

    pub(super) fn is_container_string_expr(&self, node: NodeId) -> bool {
        if let Some((container, _)) = self.associative_string_element(node) {
            return self.model.containers[container].element.is_string();
        }
        self.container_element_path(node)
            .and_then(|(container, indices)| self.container_element_type(container, indices.len()))
            .is_some_and(|element| element.is_string())
    }

    pub(super) fn lower_container_string_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        if let Some((container, key)) = self.associative_string_element(node) {
            if self.model.containers[container].element.is_string() {
                return Ok(Some(IrStringExpr::AssociativeGet {
                    container,
                    key: Box::new(self.lower_string(path, key)?),
                }));
            }
        }
        let Some((container, indices)) = self.container_element_path(node) else {
            return Ok(None);
        };
        if !self
            .container_element_type(container, indices.len())
            .is_some_and(|element| element.is_string())
        {
            return Ok(None);
        }
        let indices = self.lower_container_path_indices(path, container, indices)?;
        Ok(Some(if indices.len() == 1 {
            IrStringExpr::ContainerGet {
                container,
                index: Box::new(indices.into_iter().next().unwrap()),
            }
        } else {
            IrStringExpr::ContainerGetNested { container, indices }
        }))
    }

    pub(super) fn is_container_chandle_expr(&self, node: NodeId) -> bool {
        if let Some((container, _)) = self.associative_string_element(node) {
            return self.model.containers[container].element.is_chandle();
        }
        self.container_element_path(node)
            .and_then(|(container, indices)| self.container_element_type(container, indices.len()))
            .is_some_and(|element| element.is_chandle())
    }

    pub(super) fn lower_container_chandle_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        if let Some((container, key)) = self.associative_string_element(node) {
            if self.model.containers[container].element.is_chandle() {
                return Ok(Some(IrChandleExpr::AssociativeGet {
                    container,
                    key: Box::new(self.lower_string(path, key)?),
                }));
            }
        }
        let Some((container, indices)) = self.container_element_path(node) else {
            return Ok(None);
        };
        if !self
            .container_element_type(container, indices.len())
            .is_some_and(|element| element.is_chandle())
        {
            return Ok(None);
        }
        let indices = self.lower_container_path_indices(path, container, indices)?;
        Ok(Some(if indices.len() == 1 {
            IrChandleExpr::ContainerGet {
                container,
                index: Box::new(indices.into_iter().next().unwrap()),
            }
        } else {
            IrChandleExpr::ContainerGetNested { container, indices }
        }))
    }

    pub(super) fn lower_container_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if let Some((container, indices)) = self.container_element_path(node) {
            let element = self
                .container_element_type(container, indices.len())
                .ok_or_else(|| {
                    format!("nested container access in {path} crosses a non-container element")
                })?;
            if element.is_string() || element.is_chandle() {
                return Ok(None);
            }
            let indices = self.lower_container_path_indices(path, container, indices)?;
            let operation = if indices.len() == 1 {
                if element.is_real() {
                    IrContainerExpr::GetReal {
                        container,
                        index: Box::new(indices.into_iter().next().unwrap()),
                    }
                } else if element.is_packed() {
                    IrContainerExpr::Get {
                        container,
                        index: Box::new(indices.into_iter().next().unwrap()),
                    }
                } else {
                    return Err(format!(
                        "container element access in {path} requires a scalar value"
                    ));
                }
            } else if element.is_real() {
                IrContainerExpr::GetNestedReal { container, indices }
            } else if element.is_packed() {
                IrContainerExpr::GetNested { container, indices }
            } else {
                return Err(format!(
                    "nested container access in {path} requires a scalar value"
                ));
            };
            let (width, signed) = if element.is_real() {
                (0, false)
            } else {
                (element.width(), element.signed())
            };
            return Ok(Some(IrExpr::new(
                IrExprKind::Container(Box::new(operation)),
                width,
                signed,
                None,
            )));
        }
        let operation = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let Some(container) = self.container_of(*base) else {
                    return Ok(None);
                };
                let element = self.model.containers[container.ir].element.clone();
                if element.is_string() || element.is_chandle() {
                    return Ok(None);
                }
                match self.model.containers[container.ir].kind {
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } if element.is_real() => IrContainerExpr::GetStringReal {
                        container: container.ir,
                        key: self.lower_string(path, *index)?,
                    },
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } => IrContainerExpr::GetString {
                        container: container.ir,
                        key: self.lower_string(path, *index)?,
                    },
                    _ if element.is_real() => IrContainerExpr::GetReal {
                        container: container.ir,
                        index: Box::new(self.lower_container_index(path, *index)?),
                    },
                    _ if element.is_packed() => IrContainerExpr::Get {
                        container: container.ir,
                        index: Box::new(if matches!(
                            self.model.containers[container.ir].kind,
                            IrContainerKind::Queue { .. }
                        ) {
                            self.lower_queue_index(path, container.ir, *index)?
                        } else {
                            self.lower_container_index(path, *index)?
                        }),
                    },
                    _ => {
                        return Err(format!(
                            "nested or aggregate container element access in `{path}` is not yet a scalar expression"
                        ))
                    }
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let Some(container) = self.container_of(*base) else {
                    return Ok(None);
                };
                if indices.len() != 1 {
                    return Err(format!(
                        "multidimensional resizable container access in `{path}` is not supported"
                    ));
                }
                let element = self.model.containers[container.ir].element.clone();
                if element.is_string() || element.is_chandle() {
                    return Ok(None);
                }
                match self.model.containers[container.ir].kind {
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } if element.is_real() => IrContainerExpr::GetStringReal {
                        container: container.ir,
                        key: self.lower_string(path, indices[0])?,
                    },
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } => IrContainerExpr::GetString {
                        container: container.ir,
                        key: self.lower_string(path, indices[0])?,
                    },
                    _ if element.is_real() => IrContainerExpr::GetReal {
                        container: container.ir,
                        index: Box::new(self.lower_container_index(path, indices[0])?),
                    },
                    _ if element.is_packed() => IrContainerExpr::Get {
                        container: container.ir,
                        index: Box::new(if matches!(
                            self.model.containers[container.ir].kind,
                            IrContainerKind::Queue { .. }
                        ) {
                            self.lower_queue_index(path, container.ir, indices[0])?
                        } else {
                            self.lower_container_index(path, indices[0])?
                        }),
                    },
                    _ => {
                        return Err(format!(
                            "nested or aggregate container element access in `{path}` is not yet a scalar expression"
                        ))
                    }
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => {
                let Some(container) = self.container_of(*receiver) else {
                    return Ok(None);
                };
                let name = name.clone();
                let args = self.container_method_arguments(path, node, *receiver)?;
                if matches!(name.as_str(), "sum" | "product" | "and" | "or" | "xor")
                    && self.db.method_call_has_with_clause(node)
                {
                    let Some((callback, result_width, result_signed, result_two_state)) =
                        self.lower_container_method_callback(path, node, *receiver, container.ir)?
                    else {
                        return Err(format!(
                            "container reduction `{name}` in `{path}` has no with clause"
                        ));
                    };
                    return Ok(Some(IrExpr::new(
                        IrExprKind::Container(Box::new(IrContainerExpr::ReduceWith {
                            container: container.ir,
                            operation: match name.as_str() {
                                "sum" => IrContainerReduction::Sum,
                                "product" => IrContainerReduction::Product,
                                "and" => IrContainerReduction::BitAnd,
                                "or" => IrContainerReduction::BitOr,
                                _ => IrContainerReduction::BitXor,
                            },
                            callback,
                            result_width,
                            result_signed,
                            result_two_state,
                        })),
                        result_width,
                        result_signed,
                        None,
                    )));
                }
                if self.db.method_call_has_with_clause(node)
                    && !matches!(name.as_str(), "sum" | "product" | "and" | "or" | "xor")
                {
                    if matches!(
                        name.as_str(),
                        "find"
                            | "find_index"
                            | "find_first"
                            | "find_first_index"
                            | "find_last"
                            | "find_last_index"
                            | "min"
                            | "max"
                            | "unique"
                            | "unique_index"
                            | "sort"
                            | "rsort"
                    ) {
                        return Err(format!(
                            "array method `{name}` in `{path}` returns or mutates a container and is only valid in its statement/assignment context"
                        ));
                    }
                    return Err(format!(
                        "container method `{name}` with a `with` clause in `{path}` is not supported"
                    ));
                }
                match (name.as_str(), args.as_slice()) {
                    ("size" | "num", []) => IrContainerExpr::Size(container.ir),
                    ("sum" | "product" | "and" | "or" | "xor", []) => IrContainerExpr::Reduce {
                        container: container.ir,
                        operation: match name.as_str() {
                            "sum" => IrContainerReduction::Sum,
                            "product" => IrContainerReduction::Product,
                            "and" => IrContainerReduction::BitAnd,
                            "or" => IrContainerReduction::BitOr,
                            _ => IrContainerReduction::BitXor,
                        },
                    },
                    ("exists", [key]) => match self.model.containers[container.ir].kind {
                        IrContainerKind::Associative {
                            key: IrAssocKey::String,
                        } => IrContainerExpr::ExistsString {
                            container: container.ir,
                            key: self.lower_string(path, *key)?,
                        },
                        _ => IrContainerExpr::Exists {
                            container: container.ir,
                            key: Box::new(self.lower_container_index(path, *key)?),
                        },
                    },
                    ("pop_front", []) => IrContainerExpr::QueuePopFront(container.ir),
                    ("pop_back", []) => IrContainerExpr::QueuePopBack(container.ir),
                    ("first" | "last" | "next" | "prev", [key]) => {
                        let direction = match name.as_str() {
                            "first" => IrAssocTraversal::First,
                            "last" => IrAssocTraversal::Last,
                            "next" => IrAssocTraversal::Next,
                            _ => IrAssocTraversal::Prev,
                        };
                        if matches!(
                            self.model.containers[container.ir].kind,
                            IrContainerKind::Associative {
                                key: IrAssocKey::String
                            }
                        ) {
                            let key_object = self.object_of(path, *key).ok_or_else(|| {
                                format!(
                                    "string associative traversal key in `{path}` must be a string variable"
                                )
                            })?;
                            if self.model.objects[key_object].ty != IrObjectType::String {
                                return Err(format!(
                                    "string associative traversal key in `{path}` must be a string variable"
                                ));
                            }
                            return Ok(Some(IrExpr::new(
                                IrExprKind::Container(Box::new(
                                    IrContainerExpr::AssocTraverseString {
                                        container: container.ir,
                                        direction,
                                        key_object,
                                    },
                                )),
                                32,
                                true,
                                None,
                            )));
                        }
                        if matches!(
                            self.model.containers[container.ir].kind,
                            IrContainerKind::Associative {
                                key: IrAssocKey::Wildcard
                            }
                        ) {
                            return Err(format!(
                                "associative array `{path}` uses a wildcard index; first/last/next/prev are illegal"
                            ));
                        }
                        let (key_address, key_signal, key_width, key_signed, key_two_state) =
                            self.container_key_address(path, *key)?;
                        IrContainerExpr::AssocTraverse {
                            container: container.ir,
                            direction,
                            key_address,
                            key_signal,
                            key_width,
                            key_signed,
                            key_two_state,
                        }
                    }
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
        let (width, signed) = match &operation {
            IrContainerExpr::Size(_)
            | IrContainerExpr::AssocTraverse { .. }
            | IrContainerExpr::AssocTraverseString { .. }
            | IrContainerExpr::AssocTraverseStringLocal { .. } => (32, true),
            IrContainerExpr::Exists { .. } | IrContainerExpr::ExistsString { .. } => (32, true),
            IrContainerExpr::Reduce { container, .. } => {
                let container = &self.model.containers[*container];
                (container.element.width(), container.element.signed())
            }
            IrContainerExpr::ReduceWith {
                result_width,
                result_signed,
                ..
            } => (*result_width, *result_signed),
            IrContainerExpr::GetReal { .. } => (0, false),
            IrContainerExpr::GetStringReal { .. } => (0, false),
            _ => {
                let index = match &operation {
                    IrContainerExpr::Get { container, .. }
                    | IrContainerExpr::GetString { container, .. }
                    | IrContainerExpr::GetStringReal { container, .. } => *container,
                    IrContainerExpr::QueueFront(index)
                    | IrContainerExpr::QueueBack(index)
                    | IrContainerExpr::QueuePopFront(index)
                    | IrContainerExpr::QueuePopBack(index) => *index,
                    _ => unreachable!(),
                };
                let container = &self.model.containers[index];
                (container.element.width(), container.element.signed())
            }
        };
        Ok(Some(IrExpr::new(
            IrExprKind::Container(Box::new(operation)),
            width,
            signed,
            None,
        )))
    }

    // Fixed unpacked-array assignment (P30).

    /// A fixed-array view is represented by the complete coordinate list in
    /// logical (declared left-to-right) order.  Keeping the view as concrete
    /// coordinates lets the existing guarded ArrayRead/ArrayElem IR preserve
    /// direction, notifications, force precedence, and two-state conversion.
    fn p30_fixed_array_assignment_candidate(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Array { .. } | NodeKind::Expr(ExprKind::Ref { .. }) => {
                self.array_of(node).is_some()
            }
            NodeKind::Expr(ExprKind::PartSelect { base, .. }) => {
                self.p30_array_prefix_base(*base).is_some()
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => {
                self.p30_array_prefix_base(*base).is_some()
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => self
                .p30_array_prefix_base(*base)
                .is_some_and(|(array, consumed)| {
                    consumed.saturating_add(indices.len()) < array.dims.len()
                }),
            _ => false,
        }
    }

    fn p30_container_source(&self, node: NodeId) -> Option<ContainerInfo> {
        if let Some(container) = self.container_of(node) {
            return Some(container);
        }
        let operand = match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
            _ => None,
        }?;
        self.p30_container_source(operand)
    }

    fn p30_unwrap_cast(&self, node: NodeId) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.p30_unwrap_cast(*operand),
            _ => node,
        }
    }

    fn p30_array_prefix_base(&self, node: NodeId) -> Option<(&ArrayInfo, usize)> {
        if let Some(array) = self.array_of(node) {
            return Some((array, 0));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.p30_array_prefix_base(*operand),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (array, consumed) = self.p30_array_prefix_base(*base)?;
                let consumed = consumed.checked_add(indices.len())?;
                (consumed < array.dims.len()).then_some((array, consumed))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => {
                let (array, consumed) = self.p30_array_prefix_base(*base)?;
                let consumed = consumed.checked_add(1)?;
                (consumed < array.dims.len()).then_some((array, consumed))
            }
            _ => None,
        }
    }

    fn p30_index_expr(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<IrExpr, String> {
        if let Some((name, width, signed)) = captured.get(&node) {
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(name.clone()),
                *width,
                *signed,
                None,
            ));
        }
        let value = self.lower_expr(path, node)?;
        if value.is_real() {
            return Err(format!(
                "fixed unpacked-array index in `{path}` must be an integral expression"
            ));
        }
        let name = format!("_p30_idx_{}_{}", node.0, captured.len());
        let width = value.width;
        let signed = value.signed;
        captures.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            init: Some(Box::new(value)),
            two_state: false,
        });
        captured.insert(node, (name.clone(), width, signed));
        Ok(IrExpr::new(
            IrExprKind::LocalRead(name),
            width,
            signed,
            None,
        ))
    }

    fn p30_const_index(value: i32) -> IrExpr {
        pattern_key_expr(i128::from(value), 32, true, false)
    }

    fn p30_append_coordinates(
        dims: &[(i32, i32)],
        prefix: &[IrExpr],
        out: &mut Vec<Vec<IrExpr>>,
    ) -> Result<(), String> {
        if dims.is_empty() {
            out.push(prefix.to_vec());
            return Ok(());
        }
        let (left, right) = dims[0];
        let count = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
        for offset in 0..count {
            let offset = i32::try_from(offset)
                .map_err(|_| "fixed unpacked-array coordinate count overflows".to_string())?;
            let index = if left >= right {
                left.checked_sub(offset)
            } else {
                left.checked_add(offset)
            }
            .ok_or_else(|| "fixed unpacked-array coordinate overflows".to_string())?;
            let mut next = prefix.to_vec();
            next.push(Self::p30_const_index(index));
            Self::p30_append_coordinates(&dims[1..], &next, out)?;
        }
        Ok(())
    }

    fn p30_view_from_prefix(array: ArrayInfo, prefix: &[IrExpr]) -> Result<P30ArrayView, String> {
        if prefix.len() > array.dims.len() {
            return Err("fixed unpacked-array index rank exceeds the declared rank".to_string());
        }
        let mut coordinates = Vec::new();
        Self::p30_append_coordinates(&array.dims[prefix.len()..], prefix, &mut coordinates)?;
        Ok(P30ArrayView { array, coordinates })
    }

    fn p30_slice_view_with_prefix(
        &self,
        path: &str,
        array: ArrayInfo,
        prefix: &[IrExpr],
        left_node: NodeId,
        right_node: NodeId,
    ) -> Result<P30ArrayView, String> {
        let left = self.eval_bound_i128(left_node).map_err(|_| {
            format!("fixed unpacked-array slice bounds in `{path}` must be constant integers")
        })?;
        let right = self.eval_bound_i128(right_node).map_err(|_| {
            format!("fixed unpacked-array slice bounds in `{path}` must be constant integers")
        })?;
        let left = i32::try_from(left).map_err(|_| {
            format!("fixed unpacked-array slice left bound is out of range in `{path}`")
        })?;
        let right = i32::try_from(right).map_err(|_| {
            format!("fixed unpacked-array slice right bound is out of range in `{path}`")
        })?;
        let Some((decl_left, decl_right)) = array.dims.get(prefix.len()).copied() else {
            return Err(format!(
                "fixed unpacked-array slice has no dimension in `{path}`"
            ));
        };
        if left < decl_left.min(decl_right)
            || left > decl_left.max(decl_right)
            || right < decl_left.min(decl_right)
            || right > decl_left.max(decl_right)
        {
            return Err(format!(
                "fixed unpacked-array slice [{left}:{right}] is outside declared bounds [{decl_left}:{decl_right}] in `{path}`"
            ));
        }
        let count = (i64::from(left) - i64::from(right)).unsigned_abs();
        let step = if left >= right { -1 } else { 1 };
        let mut coordinates = Vec::new();
        for offset in 0..count {
            let offset = i32::try_from(offset)
                .map_err(|_| format!("fixed unpacked-array slice is too large in `{path}`"))?;
            let index = left
                .checked_add(step * offset)
                .ok_or_else(|| format!("fixed unpacked-array slice overflows in `{path}`"))?;
            let mut coordinate_prefix = prefix.to_vec();
            coordinate_prefix.push(Self::p30_const_index(index));
            Self::p30_append_coordinates(
                &array.dims[prefix.len() + 1..],
                &coordinate_prefix,
                &mut coordinates,
            )?;
        }
        // A range includes both endpoints.  The loop above intentionally uses
        // the absolute difference, so add the endpoint when the range is not
        // a singleton.
        let endpoint = if count == 0 { left } else { right };
        let mut coordinate_prefix = prefix.to_vec();
        coordinate_prefix.push(Self::p30_const_index(endpoint));
        Self::p30_append_coordinates(
            &array.dims[prefix.len() + 1..],
            &coordinate_prefix,
            &mut coordinates,
        )?;
        Ok(P30ArrayView { array, coordinates })
    }

    fn p30_array_prefix(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Option<(ArrayInfo, Vec<IrExpr>)>, String> {
        if let Some(array) = self.array_of(node).cloned() {
            return Ok(Some((array, Vec::new())));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.p30_array_prefix(path, *operand, captures, captured_indices)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, *base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(indices.len()) >= array.dims.len() {
                    return Ok(None);
                }
                for index in indices {
                    prefix.push(self.p30_index_expr(path, *index, captures, captured_indices)?);
                }
                Ok(Some((array, prefix)))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, *base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(1) >= array.dims.len() {
                    return Ok(None);
                }
                prefix.push(self.p30_index_expr(path, *index, captures, captured_indices)?);
                Ok(Some((array, prefix)))
            }
            _ => Ok(None),
        }
    }

    fn p30_array_view(
        &mut self,
        path: &str,
        node: NodeId,
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Option<P30ArrayView>, String> {
        let cast_operand = match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
            _ => None,
        };
        if let Some(operand) = cast_operand {
            return self.p30_array_view(path, operand, captures, captured_indices);
        }
        if let Some((array, prefix)) =
            self.p30_array_prefix(path, node, captures, captured_indices)?
        {
            return Ok(Some(Self::p30_view_from_prefix(array, &prefix)?));
        }
        enum Select {
            Slice {
                base: NodeId,
                left: NodeId,
                right: NodeId,
            },
            Partial {
                base: NodeId,
                indices: Vec<NodeId>,
            },
        }
        let select = match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => Some(Select::Slice {
                base: *base,
                left: *left,
                right: *right,
            }),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => Some(Select::Partial {
                base: *base,
                indices: indices.clone(),
            }),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some(Select::Partial {
                base: *base,
                indices: vec![*index],
            }),
            _ => None,
        };
        match select {
            Some(Select::Slice { base, left, right }) => {
                let Some((array, prefix)) =
                    self.p30_array_prefix(path, base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.p30_slice_view_with_prefix(
                    path, array, &prefix, left, right,
                )?))
            }
            Some(Select::Partial { base, indices }) => {
                let Some((array, mut prefix)) =
                    self.p30_array_prefix(path, base, captures, captured_indices)?
                else {
                    return Ok(None);
                };
                if prefix.len().saturating_add(indices.len()) >= array.dims.len() {
                    return Ok(None);
                }
                for index in indices {
                    prefix.push(self.p30_index_expr(path, index, captures, captured_indices)?);
                }
                Ok(Some(Self::p30_view_from_prefix(array, &prefix)?))
            }
            None => Ok(None),
        }
    }

    fn p30_pattern_level(
        &self,
        path: &str,
        node: NodeId,
        bounds: (i32, i32),
    ) -> Result<Vec<NodeId>, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(node)
        else {
            return Err(format!(
                "fixed unpacked-array assignment pattern in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "fixed unpacked-array assignment pattern in `{path}` is not an assignment pattern"
            ));
        }
        let count = usize::try_from((i64::from(bounds.0) - i64::from(bounds.1)).unsigned_abs() + 1)
            .map_err(|_| format!("fixed unpacked-array pattern is too large in `{path}`"))?;
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
        }
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        if !tagged {
            if operands.len() != count {
                return Err(format!(
                    "fixed unpacked-array assignment pattern in `{path}` has {} positional values; expected {count}",
                    operands.len()
                ));
            }
            return Ok(operands);
        }
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            return Err(format!(
                "mixed positional and keyed fixed unpacked-array assignment pattern in `{path}` is not supported"
            ));
        }
        let mut explicit = HashMap::<usize, NodeId>::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern { key, value, .. }) = self.kind(operand)
            else {
                unreachable!();
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("fixed unpacked-array pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("fixed unpacked-array pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in fixed unpacked-array pattern in `{path}`"
                    ));
                }
                continue;
            }
            let index = parse_pattern_i128(key).ok_or_else(|| {
                format!(
                    "fixed unpacked-array pattern key `{key}` is not a constant index in `{path}`"
                )
            })?;
            let index = i32::try_from(index).map_err(|_| {
                format!("fixed unpacked-array pattern index `{key}` is out of range in `{path}`")
            })?;
            let (left, right) = bounds;
            let Some(offset) = (if left >= right {
                left.checked_sub(index)
            } else {
                index.checked_sub(left)
            })
            .filter(|offset| *offset >= 0) else {
                return Err(format!(
                    "fixed unpacked-array pattern index `{key}` is outside [{left}:{right}] in `{path}`"
                ));
            };
            let offset = usize::try_from(offset).map_err(|_| {
                format!("fixed unpacked-array pattern index `{key}` is out of range in `{path}`")
            })?;
            if offset >= count || explicit.insert(offset, value).is_some() {
                return Err(format!(
                    "duplicate fixed unpacked-array pattern index `{key}` in `{path}`"
                ));
            }
        }
        (0..count)
            .map(|offset| {
                explicit.get(&offset).copied().or(default).ok_or_else(|| {
                    format!(
                        "fixed unpacked-array pattern does not cover offset {offset} in `{path}`"
                    )
                })
            })
            .collect()
    }

    fn p30_pattern_values(
        &self,
        path: &str,
        node: NodeId,
        dims: &[(i32, i32)],
    ) -> Result<Vec<NodeId>, String> {
        let Some((bounds, rest)) = dims.split_first() else {
            return Ok(vec![node]);
        };
        let values = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern =>
            {
                self.p30_pattern_level(path, node, *bounds)?
            }
            _ => {
                let count = dims[1..]
                    .iter()
                    .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
                    .try_fold(1u64, |total, extent| total.checked_mul(extent))
                    .ok_or_else(|| {
                        format!("fixed unpacked-array pattern is too large in `{path}`")
                    })?;
                let count = usize::try_from(count).map_err(|_| {
                    format!("fixed unpacked-array pattern is too large in `{path}`")
                })?;
                let total = count
                    .checked_mul(
                        usize::try_from((i64::from(bounds.0) - i64::from(bounds.1)).unsigned_abs())
                            .map_err(|_| {
                                format!("fixed unpacked-array pattern is too large in `{path}`")
                            })?
                            .saturating_add(1),
                    )
                    .ok_or_else(|| {
                        format!("fixed unpacked-array pattern is too large in `{path}`")
                    })?;
                return Ok(std::iter::repeat_n(node, total).collect());
            }
        };
        if rest.is_empty() {
            return Ok(values);
        }
        let mut flattened = Vec::new();
        for value in values {
            flattened.extend(self.p30_pattern_values(path, value, rest)?);
        }
        Ok(flattened)
    }

    fn p30_capture_value(
        &self,
        lhs: NodeId,
        rhs: NodeId,
        ordinal: usize,
        value: IrExpr,
        captures: &mut Vec<IrStmt>,
    ) -> IrExpr {
        let name = format!("_p30_value_{}_{}_{}", lhs.0, rhs.0, ordinal);
        let width = value.width;
        let signed = value.signed;
        captures.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            init: Some(Box::new(value)),
            two_state: false,
        });
        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
    }

    fn p30_lower_source_values(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        target_dims: &[(i32, i32)],
        captures: &mut Vec<IrStmt>,
        captured_indices: &mut HashMap<NodeId, (String, u32, bool)>,
    ) -> Result<Vec<IrExpr>, String> {
        let source_node = self.p30_unwrap_cast(rhs);
        let concat = match self.kind(source_node) {
            NodeKind::Expr(ExprKind::Operation {
                op,
                operands,
                reordered,
                ..
            }) if *op == Operation::Concat => Some((operands.clone(), *reordered)),
            _ => None,
        };
        if let Some((mut operands, reordered)) = concat {
            if reordered {
                operands.reverse();
            }
            let mut values = Vec::new();
            for operand in operands {
                values.extend(self.p30_lower_source_values(
                    path,
                    lhs,
                    operand,
                    target_dims,
                    captures,
                    captured_indices,
                )?);
            }
            return Ok(values);
        }
        if matches!(
            self.kind(source_node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                ..
            })
        ) {
            let nodes = self.p30_pattern_values(path, source_node, target_dims)?;
            let mut values = Vec::with_capacity(nodes.len());
            for (ordinal, node) in nodes.into_iter().enumerate() {
                let value = self.lower_expr(path, node)?;
                values.push(self.p30_capture_value(lhs, rhs, ordinal, value, captures));
            }
            return Ok(values);
        }
        if let Some(view) = self.p30_array_view(path, rhs, captures, captured_indices)? {
            let mut values = Vec::with_capacity(view.coordinates.len());
            for (ordinal, coordinates) in view.coordinates.into_iter().enumerate() {
                let value = IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(view.array.ir),
                        indices: coordinates,
                        elem_sel: IrElemSel::Whole,
                    },
                    view.array.elem_width,
                    view.array.signed,
                    None,
                );
                values.push(self.p30_capture_value(lhs, rhs, ordinal, value, captures));
            }
            return Ok(values);
        }
        if self.p30_container_source(rhs).is_some() {
            return Err(format!(
                "dynamic or queue array to fixed unpacked-array assignment in `{path}` requires a runtime-compatible fixed size"
            ));
        }
        Err(format!(
            "fixed unpacked-array assignment in `{path}` requires a compatible fixed array, slice, concatenation, or assignment pattern"
        ))
    }

    fn p30_target_pattern_dims(&self, node: NodeId, array: &ArrayInfo) -> Vec<(i32, i32)> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) => {
                let mut dims = Vec::with_capacity(array.dims.len());
                let bounds = self
                    .eval_bound_i128(*left)
                    .ok()
                    .zip(self.eval_bound_i128(*right).ok())
                    .and_then(|(left, right)| {
                        Some((i32::try_from(left).ok()?, i32::try_from(right).ok()?))
                    })
                    .unwrap_or((0, 0));
                let consumed = self
                    .p30_array_prefix_base(*base)
                    .map(|(_, consumed)| consumed)
                    .unwrap_or(0);
                dims.push(bounds);
                dims.extend_from_slice(&array.dims[consumed.saturating_add(1)..]);
                dims
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. })
                if indices.len() < array.dims.len() =>
            {
                array.dims[indices.len()..].to_vec()
            }
            NodeKind::Expr(ExprKind::BitSelect { .. }) if array.dims.len() > 1 => {
                array.dims[1..].to_vec()
            }
            _ => array.dims.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_p30_container_to_fixed(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        target: P30ArrayView,
        source: ContainerInfo,
        mut captures: Vec<IrStmt>,
    ) -> Result<IrStmt, String> {
        let kind = self.model.containers[source.ir].kind.clone();
        if !matches!(
            kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "associative array to fixed unpacked-array assignment in `{path}` is not supported"
            ));
        }
        let source_type = &self.model.containers[source.ir].element;
        let (source_width, source_signed) = (source_type.width(), source_type.signed());
        if source_width == 0 {
            return Err(format!(
                "real dynamic or queue array to fixed unpacked-array assignment in `{path}` is not supported"
            ));
        }
        let destination_count = u32::try_from(target.coordinates.len())
            .map_err(|_| format!("fixed unpacked-array assignment is too large in `{path}`"))?;
        let size = IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(source.ir))),
            32,
            true,
            None,
        );
        let expected = pattern_key_expr(i128::from(destination_count), 32, true, false);
        let condition = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Eq,
                a: Box::new(size),
                b: Box::new(expected),
            },
            1,
            false,
            None,
        );
        let destination_two_state = self.model.arrays[target.array.ir].two_state;
        let target_array = target.array;
        let target_coordinates = target.coordinates;
        let mut then_body = Vec::with_capacity(target_coordinates.len() * 2);
        for (ordinal, coordinates) in target_coordinates.into_iter().enumerate() {
            let index = pattern_key_expr(
                i128::try_from(ordinal).unwrap_or(i128::MAX),
                32,
                true,
                false,
            );
            let value = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Get {
                    container: source.ir,
                    index: Box::new(index),
                })),
                source_width,
                source_signed,
                None,
            );
            let value = self.p30_capture_value(lhs, rhs, ordinal, value, &mut then_body);
            let value = if target_array.real {
                IrExpr::new(
                    IrExprKind::CastToReal {
                        a: Box::new(value),
                        shortreal: target_array.shortreal,
                    },
                    0,
                    true,
                    None,
                )
            } else {
                ir_to_storage(
                    value,
                    target_array.elem_width,
                    target_array.signed,
                    destination_two_state,
                )?
            };
            then_body.push(IrStmt::Assign {
                lhs: IrLhs::ArrayElem {
                    arr: self.reference_array(target_array.ir),
                    indices: coordinates,
                    elem_sel: IrElemSel::Whole,
                },
                rhs: value,
                nba: !blocking,
            });
        }
        let mismatch = IrExpr::new(
            IrExprKind::Verbatim {
                code: "({ fprintf(stderr, \"fixed unpacked-array assignment size mismatch\\n\"); abort(); sv4_from_u64(0, 1, 0); })".to_owned(),
                width: 1,
                signed: false,
            },
            1,
            false,
            None,
        );
        captures.push(IrStmt::If {
            cond: condition,
            then_: then_body,
            els: Some(vec![IrStmt::DeclLocal {
                name: format!("_p30_size_error_{}_{}", lhs.0, rhs.0),
                width: 1,
                signed: false,
                init: Some(Box::new(mismatch)),
                two_state: false,
            }]),
        });
        Ok(IrStmt::Block(captures))
    }

    fn lower_p30_fixed_array_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let mut captures = Vec::new();
        let mut captured_indices = HashMap::new();
        let Some(target) = self.p30_array_view(path, lhs, &mut captures, &mut captured_indices)?
        else {
            return Ok(None);
        };
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to a fixed unpacked array in `{path}` is not supported"
            ));
        }
        if let Some(source) = self.p30_container_source(rhs) {
            return Ok(Some(self.lower_p30_container_to_fixed(
                path, lhs, rhs, blocking, target, source, captures,
            )?));
        }
        let target_dims = self.p30_target_pattern_dims(lhs, &target.array);
        let values = self.p30_lower_source_values(
            path,
            lhs,
            rhs,
            &target_dims,
            &mut captures,
            &mut captured_indices,
        )?;
        if values.len() != target.coordinates.len() {
            return Err(format!(
                "fixed unpacked-array assignment in `{path}` has {} source elements; destination requires {}",
                values.len(),
                target.coordinates.len()
            ));
        }
        let target_array = target.array;
        let target_coordinates = target.coordinates;
        let destination_two_state = self.model.arrays[target_array.ir].two_state;
        for (value, coordinates) in values.into_iter().zip(target_coordinates) {
            let value = if target_array.real {
                if value.is_real() {
                    if target_array.shortreal {
                        IrExpr::new(
                            IrExprKind::CastToReal {
                                a: Box::new(value),
                                shortreal: true,
                            },
                            0,
                            true,
                            None,
                        )
                    } else {
                        value
                    }
                } else {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: target_array.shortreal,
                        },
                        0,
                        true,
                        None,
                    )
                }
            } else {
                ir_to_storage(
                    value,
                    target_array.elem_width,
                    target_array.signed,
                    destination_two_state,
                )?
            };
            let lhs = IrLhs::ArrayElem {
                arr: self.reference_array(target_array.ir),
                indices: coordinates,
                elem_sel: IrElemSel::Whole,
            };
            captures.push(IrStmt::Assign {
                lhs,
                rhs: value,
                nba: !blocking,
            });
        }
        Ok(Some(IrStmt::Block(captures)))
    }

    pub(super) fn lower_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
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

    pub(super) fn lower_container_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => (name.clone(), *receiver),
            _ => return Ok(None),
        };
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
}

fn parse_pattern_i128(key: &str) -> Option<i128> {
    let key = key.trim();
    let key = key
        .strip_prefix('[')
        .and_then(|key| key.strip_suffix(']'))
        .unwrap_or(key)
        .trim()
        .replace('_', "");
    if let Some((width, literal)) = key.split_once('\'') {
        let _ = width.parse::<u32>().ok()?;
        let (base, digits) = literal.split_at(1);
        let radix = match base {
            "b" | "B" => 2,
            "o" | "O" => 8,
            "d" | "D" => 10,
            "h" | "H" => 16,
            _ => return None,
        };
        let sign = digits.starts_with('-');
        let digits = digits.trim_start_matches('-');
        let value = i128::from_str_radix(digits, radix).ok()?;
        return Some(if sign { -value } else { value });
    }
    key.parse::<i128>().ok()
}

fn parse_pattern_string_key(key: &str) -> Option<Vec<u8>> {
    let key = key.trim();
    let key = key.strip_prefix('"')?.strip_suffix('"')?;
    decode_verilog_string(key).ok()
}

fn pattern_key_expr(index: i128, width: u32, signed: bool, _two_state: bool) -> IrExpr {
    let limbs = width.div_ceil(64) as usize;
    let raw = index as u128;
    let mut bits = vec![0; limbs];
    if let Some(low) = bits.get_mut(0) {
        *low = raw as u64;
    }
    if let Some(high) = bits.get_mut(1) {
        *high = (raw >> 64) as u64;
    }
    if !width.is_multiple_of(64) {
        if let Some(high) = bits.last_mut() {
            *high &= (1u64 << (width % 64)) - 1;
        }
    }
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits,
            x: vec![0; limbs],
            z: vec![0; limbs],
            width,
            signed,
            real: None,
            fill: None,
        }),
        width,
        signed,
        None,
    )
}
