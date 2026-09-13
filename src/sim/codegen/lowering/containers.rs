//! Lowering for resizable unpacked containers.

use super::*;
use crate::sim::ir::{IrContainerReduction, IrObjectType, IrStringExpr};

impl<'a> Codegen<'a> {
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
                self.lower_container_pattern(&path, container, initializer, descriptor.as_ref())?;
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
                &key_type,
                element_descriptor,
                element_two_state,
            ) {
                return Err(format!(
                    "resizable container assignment pattern key `{key}` has no matching index or type in `{path}`"
                ));
            }
            if type_values.iter().any(|(previous, _)| {
                super::collection::pattern_key_types_equal(previous, &key_type)
            }) {
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
        let mut seen_integral = Vec::<i128>::new();
        let mut seen_string = Vec::<Vec<u8>>::new();
        let mut captures = Vec::new();
        let mut captured = HashMap::<NodeId, (String, u32, bool)>::new();
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
                return Err(format!(
                    "default key for associative array assignment pattern in `{path}` is not supported"
                ));
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
                    writes.push(IrStmt::Container(IrContainerStmt::SetString {
                        container,
                        key: IrStringExpr::Literal(bytes),
                        value: lowered,
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
                    writes.push(IrStmt::Container(IrContainerStmt::Set {
                        container,
                        index: pattern_key_expr(index, width, signed, two_state),
                        value: lowered,
                    }));
                }
                _ => unreachable!(),
            }
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

    fn lower_container_value(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        let IrType::Packed {
            width,
            signed,
            two_state,
        } = self.model.containers[container].element
        else {
            return Err("container element type is not packed".into());
        };
        let value = self.lower_expr(path, node)?;
        let value = apply_assignment_expression_width(value, width);
        ir_to_storage(value, width, signed, two_state)
    }

    fn lower_queue_method_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, 32, true, true)
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
                if let Some((name, width, signed, two_state, _shortreal)) = function.locals.get(&target) {
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

    pub(super) fn lower_container_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let operation = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let Some(container) = self.container_of(*base) else {
                    return Ok(None);
                };
                match self.model.containers[container.ir].kind {
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } => IrContainerExpr::GetString {
                        container: container.ir,
                        key: self.lower_string(path, *index)?,
                    },
                    _ => IrContainerExpr::Get {
                        container: container.ir,
                        index: Box::new(self.lower_container_index(path, *index)?),
                    },
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
                match self.model.containers[container.ir].kind {
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    } => IrContainerExpr::GetString {
                        container: container.ir,
                        key: self.lower_string(path, indices[0])?,
                    },
                    _ => IrContainerExpr::Get {
                        container: container.ir,
                        index: Box::new(self.lower_container_index(path, indices[0])?),
                    },
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
                if self.db.method_call_has_with_clause(node) {
                    return Err(format!(
                        "container method `{name}` with a `with` clause in `{path}` is not supported"
                    ));
                }
                let args = self.container_method_arguments(path, node, *receiver)?;
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
            | IrContainerExpr::AssocTraverseString { .. } => (32, true),
            IrContainerExpr::Exists { .. } | IrContainerExpr::ExistsString { .. } => (32, true),
            IrContainerExpr::Reduce { container, .. } => {
                let container = &self.model.containers[*container];
                (container.element.width(), container.element.signed())
            }
            _ => {
                let index = match &operation {
                    IrContainerExpr::Get { container, .. }
                    | IrContainerExpr::GetString { container, .. } => *container,
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

    pub(super) fn lower_container_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
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
            let operation = match self.model.containers[container.ir].kind {
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                } => IrContainerStmt::SetString {
                    container: container.ir,
                    key: self.lower_string(path, index)?,
                    value: self.lower_container_value(path, container.ir, rhs)?,
                },
                _ => IrContainerStmt::Set {
                    container: container.ir,
                    index: self.lower_container_index(path, index)?,
                    value: self.lower_container_value(path, container.ir, rhs)?,
                },
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
        if self.db.method_call_has_with_clause(node) {
            return Err(format!(
                "container method `{name}` with a `with` clause in `{path}` is not supported"
            ));
        }
        let args = self.container_method_arguments(path, node, receiver)?;
        let operation = match (name.as_str(), args.as_slice()) {
            ("delete", []) => IrContainerStmt::Delete(container.ir),
            ("delete", [index]) => match self.model.containers[container.ir].kind {
                IrContainerKind::Queue { .. } => IrContainerStmt::DeleteIndex {
                    container: container.ir,
                    index: self.lower_queue_method_index(path, *index)?,
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
            ("push_front", [value]) => IrContainerStmt::QueuePushFront {
                container: container.ir,
                value: self.lower_container_value(path, container.ir, *value)?,
            },
            ("push_back", [value]) => IrContainerStmt::QueuePushBack {
                container: container.ir,
                value: self.lower_container_value(path, container.ir, *value)?,
            },
            ("insert", [index, value]) => IrContainerStmt::QueueInsert {
                container: container.ir,
                index: self.lower_queue_method_index(path, *index)?,
                value: self.lower_container_value(path, container.ir, *value)?,
            },
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
    if width % 64 != 0 {
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
