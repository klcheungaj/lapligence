//! Queries.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn is_container_string_expr(&self, node: NodeId) -> bool {
        if let Some((container, _)) = self.queue_pop_call(node) {
            return self.model.containers[container].element.is_string();
        }
        if let Some((container, _)) = self.associative_string_element(node) {
            return self.model.containers[container].element.is_string();
        }
        self.container_element_path(node)
            .and_then(|(container, indices)| self.container_element_type(container, indices.len()))
            .is_some_and(|element| element.is_string())
    }

    pub(in super::super) fn lower_container_string_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        if let Some((container, back)) = self.queue_pop_call(node) {
            if self.model.containers[container].element.is_string() {
                return Ok(Some(IrStringExpr::QueuePop { container, back }));
            }
        }
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

    /// The identity stored in one element of a resizable event container.
    /// A missing or invalid element yields null (SV Table 7-1), which the
    /// runtime treats as the null event handle.
    pub(in super::super) fn container_event_handle(
        &mut self,
        scope_path: &str,
        container: usize,
        indices: &[NodeId],
    ) -> Result<IrChandleExpr, String> {
        let name = self.model.containers[container].c_name.clone();
        let Some(&first) = indices.first() else {
            return Err(format!(
                "event container `{name}` in `{scope_path}` must be selected to one element"
            ));
        };
        if !self
            .container_element_type(container, indices.len())
            .is_some_and(|element| element.is_event())
        {
            return Err(format!(
                "event container select of `{name}` in `{scope_path}` does not name one event element"
            ));
        }
        if matches!(
            self.model.containers[container].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        ) {
            if indices.len() != 1 {
                return Err(format!(
                    "nested string-keyed event container `{name}` in `{scope_path}` is not supported"
                ));
            }
            return Ok(IrChandleExpr::AssociativeGet {
                container,
                key: Box::new(self.lower_string(scope_path, first)?),
            });
        }
        let mut indices =
            self.lower_container_path_indices(scope_path, container, indices.to_vec())?;
        Ok(if indices.len() == 1 {
            IrChandleExpr::ContainerGet {
                container,
                index: Box::new(indices.remove(0)),
            }
        } else {
            IrChandleExpr::ContainerGetNested { container, indices }
        })
    }

    /// A `pop_front()`/`pop_back()` call on a queue: its container and
    /// whether it removes the back element.
    pub(in super::super) fn queue_pop_call(&self, node: NodeId) -> Option<(usize, bool)> {
        let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        else {
            return None;
        };
        let back = match name.as_str() {
            "pop_front" => false,
            "pop_back" => true,
            _ => return None,
        };
        let container = self.container_of(*receiver)?.ir;
        matches!(
            self.model.containers[container].kind,
            IrContainerKind::Queue { .. }
        )
        .then_some((container, back))
    }

    /// Lower the value stored into a handle element. Event elements store
    /// the synchronization object of an event handle (or null); object and
    /// chandle elements store the handle itself. Neither copies the referent.
    pub(super) fn lower_container_handle(
        &mut self,
        path: &str,
        element: &IrContainerElement,
        value: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if element.is_process() {
            return Ok(IrChandleExpr::Process(Box::new(
                self.lower_process(path, value)?,
            )));
        }
        if !element.is_event() {
            // A mailbox constructor needs the element type of the mailbox
            // specialization, which only the mailbox lowering resolves.
            if matches!(self.kind(value), NodeKind::Expr(ExprKind::NewClass { .. }))
                && self.is_mailbox_expr(path, value)
            {
                let element = self.mailbox_element_for_decl(value);
                return Ok(IrChandleExpr::Mailbox(Box::new(
                    self.lower_mailbox_expr(path, value, element)?,
                )));
            }
            return self.lower_chandle(path, value);
        }
        if self.is_null_event_expression(value) {
            return Ok(IrChandleExpr::Null);
        }
        if let Some((container, back)) = self
            .queue_pop_call(value)
            .filter(|(container, _)| self.model.containers[*container].element.is_event())
        {
            return Ok(IrChandleExpr::QueuePop { container, back });
        }
        let Some(target) = self.event_target_of(value) else {
            return Err(format!(
                "event container element in `{path}` requires an event handle or null"
            ));
        };
        let event = self.event_ref_of(&target, path)?;
        Ok(IrChandleExpr::EventObject(Box::new(event)))
    }

    /// Whether `node` reads or pops a `process` element of container
    /// storage (lowered through `lower_process`, never as a chandle).
    pub(in super::super) fn is_container_process_expr(&self, node: NodeId) -> bool {
        if let Some((container, _)) = self.queue_pop_call(node) {
            return self.model.containers[container].element.is_process();
        }
        if let Some((container, _)) = self.associative_string_element(node) {
            return self.model.containers[container].element.is_process();
        }
        self.container_element_path(node)
            .and_then(|(container, indices)| self.container_element_type(container, indices.len()))
            .is_some_and(|element| element.is_process())
    }

    /// The element handle read by a process container access, wrapped by
    /// the caller as `IrProcessExpr::Handle`.
    pub(in super::super) fn lower_container_process_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        if !self.is_container_process_expr(node) {
            return Ok(None);
        }
        if let Some((container, back)) = self.queue_pop_call(node) {
            return Ok(Some(IrChandleExpr::QueuePop { container, back }));
        }
        if let Some((container, key)) = self.associative_string_element(node) {
            return Ok(Some(IrChandleExpr::AssociativeGet {
                container,
                key: Box::new(self.lower_string(path, key)?),
            }));
        }
        let Some((container, indices)) = self.container_element_path(node) else {
            return Ok(None);
        };
        let mut indices = self.lower_container_path_indices(path, container, indices)?;
        Ok(Some(if indices.len() == 1 {
            IrChandleExpr::ContainerGet {
                container,
                index: Box::new(indices.remove(0)),
            }
        } else {
            IrChandleExpr::ContainerGetNested { container, indices }
        }))
    }

    pub(in super::super) fn is_container_chandle_expr(&self, node: NodeId) -> bool {
        if let Some((container, _)) = self.queue_pop_call(node) {
            return self.model.containers[container].element.is_object_handle();
        }
        if let Some((container, _)) = self.associative_string_element(node) {
            return self.model.containers[container].element.is_object_handle();
        }
        self.container_element_path(node)
            .and_then(|(container, indices)| self.container_element_type(container, indices.len()))
            .is_some_and(|element| element.is_object_handle())
    }

    pub(in super::super) fn lower_container_chandle_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        if let Some((container, back)) = self.queue_pop_call(node) {
            if self.model.containers[container].element.is_object_handle() {
                return Ok(Some(IrChandleExpr::QueuePop { container, back }));
            }
        }
        if let Some((container, key)) = self.associative_string_element(node) {
            if self.model.containers[container].element.is_object_handle() {
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
            .is_some_and(|element| element.is_object_handle())
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

    pub(in super::super) fn lower_container_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if let Some(value) = self.lower_container_flattened_select(path, node)? {
            return Ok(Some(value));
        }
        if let Some((container, indices)) = self.container_element_path(node) {
            let element = self
                .container_element_type(container, indices.len())
                .ok_or_else(|| {
                    format!("nested container access in {path} crosses a non-container element")
                })?;
            if element.is_string() || element.is_handle() {
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
                if element.is_string() || element.is_handle() {
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
                if element.is_string() || element.is_handle() {
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
                ..
            } => {
                if let Some((container, indices)) = self.nested_container_receiver(*receiver) {
                    if !matches!(name.as_str(), "size" | "num")
                        || !self.node(node).children[1..].is_empty()
                    {
                        return Err(format!(
                            "method `{name}` of a nested container element in `{path}` is not supported; only `size()` is"
                        ));
                    }
                    let indices = self.lower_container_path_indices(path, container, indices)?;
                    return Ok(Some(IrExpr::new(
                        IrExprKind::Container(Box::new(IrContainerExpr::NestedSize {
                            container,
                            indices,
                        })),
                        32,
                        true,
                        None,
                    )));
                }
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
                    ("pop_front" | "pop_back", [])
                        if !self.model.containers[container.ir].element.is_packed()
                            && !self.model.containers[container.ir].element.is_real() =>
                    {
                        // String, handle and record results have typed owners.
                        return Ok(None);
                    }
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
}
