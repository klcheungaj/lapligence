//! Lowering for resizable unpacked containers.

use super::*;
use crate::sim::ir::IrObjectType;

impl<'a> Codegen<'a> {
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
            if let Some(local) = self.proc_locals.get(&target) {
                return Ok((
                    format!("&{}", local.c_name),
                    None,
                    local.width,
                    local.signed,
                    local.two_state,
                ));
            }
            if let Some(function) = &self.func {
                if let Some((name, width, signed, two_state)) = function.locals.get(&target) {
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
                let args = self.node(node).children[1..].to_vec();
                match (name.as_str(), args.as_slice()) {
                    ("size" | "num", []) => IrContainerExpr::Size(container.ir),
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
        op: i32,
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
            if !matches!(op, 0 | vpi::vpiAssignmentOp) {
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
        if !matches!(op, 0 | vpi::vpiAssignmentOp) {
            return Err(format!(
                "compound assignment to resizable container in `{path}` is not supported"
            ));
        }
        if let NodeKind::Expr(ExprKind::Operation {
            op: pattern_op,
            operands,
            reordered,
        }) = self.kind(rhs)
        {
            if *pattern_op == vpi::vpiAssignmentPatternOp {
                if matches!(
                    self.model.containers[dst.ir].kind,
                    IrContainerKind::Associative { .. }
                ) {
                    return Err(format!(
                        "positional assignment pattern for associative array in `{path}` is not supported"
                    ));
                }
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                if operands.iter().any(|operand| {
                    matches!(
                        self.kind(*operand),
                        NodeKind::Expr(ExprKind::TaggedPattern { .. })
                    )
                }) {
                    return Err(format!(
                        "keyed/default resizable-container assignment pattern in `{path}` is not supported"
                    ));
                }
                let values = operands
                    .into_iter()
                    .map(|operand| self.lower_container_value(path, dst.ir, operand))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(Some(IrStmt::Container(IrContainerStmt::AssignValues {
                    container: dst.ir,
                    values,
                })));
            }
        }
        if let NodeKind::MethodCall {
            name,
            receiver: None,
        } = self.kind(rhs)
        {
            if name == "new" {
                if !matches!(self.model.containers[dst.ir].kind, IrContainerKind::Dynamic) {
                    return Err(format!("new[] target in `{path}` is not a dynamic array"));
                }
                let args = self.node(rhs).children.clone();
                if !(1..=2).contains(&args.len()) {
                    return Err(format!(
                        "dynamic-array new[] in `{path}` has {} arguments; expected size and optional initializer",
                        args.len()
                    ));
                }
                let size = ir_to_storage(self.lower_expr(path, args[0])?, 64, true, true)?;
                let initializer = args
                    .get(1)
                    .map(|node| {
                        self.container_of(*node)
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
        let args = self.node(node).children[1..].to_vec();
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
