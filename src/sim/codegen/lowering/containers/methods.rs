//! Methods.

use super::*;

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
