//! Dynamically sized unpacked container storage and operations.

use super::{IrExpr, IrObjectType, IrStringExpr, IrType, IrValidationError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrAssocKey {
    Wildcard,
    Integral {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrContainerKind {
    Dynamic,
    Queue { maximum_elements: Option<u64> },
    Associative { key: IrAssocKey },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrContainer {
    pub c_name: String,
    pub element: IrType,
    pub kind: IrContainerKind,
}

/// Container expressions return packed element values or packed method status.
#[derive(Clone, Debug, PartialEq)]
pub enum IrContainerExpr {
    Size(usize),
    Get {
        container: usize,
        index: Box<IrExpr>,
    },
    GetString {
        container: usize,
        key: IrStringExpr,
    },
    Exists {
        container: usize,
        key: Box<IrExpr>,
    },
    ExistsString {
        container: usize,
        key: IrStringExpr,
    },
    AssocTraverse {
        container: usize,
        direction: IrAssocTraversal,
        /// Complete C `sv4_t*` address, like [`super::IrLhs::WholeRef`].
        key_address: String,
        key_signal: Option<usize>,
        key_width: u32,
        key_signed: bool,
        key_two_state: bool,
    },
    AssocTraverseString {
        container: usize,
        direction: IrAssocTraversal,
        key_object: usize,
    },
    QueueFront(usize),
    QueueBack(usize),
    QueuePopFront(usize),
    QueuePopBack(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrAssocTraversal {
    First,
    Last,
    Next,
    Prev,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrContainerStmt {
    DynamicNew {
        container: usize,
        size: IrExpr,
        initializer: Option<usize>,
    },
    Copy {
        dst: usize,
        src: usize,
    },
    AssignValues {
        container: usize,
        values: Vec<IrExpr>,
    },
    Delete(usize),
    Set {
        container: usize,
        index: IrExpr,
        value: IrExpr,
    },
    SetString {
        container: usize,
        key: IrStringExpr,
        value: IrExpr,
    },
    QueuePushFront {
        container: usize,
        value: IrExpr,
    },
    QueuePushBack {
        container: usize,
        value: IrExpr,
    },
    QueueInsert {
        container: usize,
        index: IrExpr,
        value: IrExpr,
    },
    DeleteIndex {
        container: usize,
        index: IrExpr,
    },
    DeleteString {
        container: usize,
        key: IrStringExpr,
    },
}

impl IrContainerExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), IrValidationError> {
        let mut has_real = false;
        self.expressions(&mut |expr| has_real |= expr.is_real());
        if has_real {
            return Err(IrValidationError::new(
                "container",
                "container operation requires packed integral operands",
            ));
        }
        let (index, expected) = match self {
            Self::Size(index) => (*index, None),
            Self::Get { container, .. } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "string-keyed associative read requires a string expression",
                    ));
                }
                return Ok(());
            }
            Self::GetString { container, key } => {
                string_container(model, *container)?;
                key.validate(model, string_return)?;
                return Ok(());
            }
            Self::Exists { container, .. } => {
                let container = container_kind(model, *container, Some("associative"))?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "string-keyed associative query requires a string expression",
                    ));
                }
                return Ok(());
            }
            Self::ExistsString { container, key } => {
                string_container(model, *container)?;
                key.validate(model, string_return)?;
                return Ok(());
            }
            Self::AssocTraverse {
                container,
                key_address,
                key_signal,
                key_width,
                key_signed,
                key_two_state,
                ..
            } => {
                let container = container_kind(model, *container, Some("associative"))?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::Integral { .. }
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "associative traversal requires a declared integral index type",
                    ));
                }
                if key_address.is_empty() {
                    return Err(IrValidationError::new(
                        "container",
                        "associative traversal key address is empty",
                    ));
                }
                let IrContainerKind::Associative {
                    key:
                        IrAssocKey::Integral {
                            width,
                            signed,
                            two_state,
                        },
                } = &container.kind
                else {
                    unreachable!()
                };
                if (*key_width, *key_signed, *key_two_state) != (*width, *signed, *two_state) {
                    return Err(IrValidationError::new(
                        "container",
                        "associative traversal key type does not match its index type",
                    ));
                }
                if let Some(signal) = key_signal {
                    let Some(signal) = model.signals.get(*signal) else {
                        return Err(IrValidationError::new(
                            "container",
                            "associative traversal key signal is out of bounds",
                        ));
                    };
                    if (signal.ty.width(), signal.ty.signed(), signal.ty.two_state())
                        != (*key_width, *key_signed, *key_two_state)
                    {
                        return Err(IrValidationError::new(
                            "container",
                            "associative traversal signal metadata is inconsistent",
                        ));
                    }
                }
                return Ok(());
            }
            Self::AssocTraverseString {
                container,
                key_object,
                ..
            } => {
                string_container(model, *container)?;
                if !matches!(model.objects.get(*key_object), Some(object) if object.ty == IrObjectType::String)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "string associative traversal requires string variable storage",
                    ));
                }
                return Ok(());
            }
            Self::QueueFront(index)
            | Self::QueueBack(index)
            | Self::QueuePopFront(index)
            | Self::QueuePopBack(index) => (*index, Some("queue")),
        };
        container_kind(model, index, expected).map(|_| ())
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Get { index, .. } | Self::Exists { key: index, .. } => visit(index),
            Self::GetString { key, .. } | Self::ExistsString { key, .. } => key.expressions(visit),
            _ => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Get { index, .. } | Self::Exists { key: index, .. } => visit(index),
            Self::GetString { key, .. } | Self::ExistsString { key, .. } => {
                key.expressions_mut(visit)
            }
            _ => {}
        }
    }

    pub(in crate::sim) fn traversal_signal(&self) -> Option<usize> {
        match self {
            Self::AssocTraverse { key_signal, .. } => *key_signal,
            _ => None,
        }
    }
}

impl IrContainerStmt {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), IrValidationError> {
        let mut has_real = false;
        self.expressions(&mut |expr| has_real |= expr.is_real());
        if has_real {
            return Err(IrValidationError::new(
                "container",
                "container operation requires packed integral operands",
            ));
        }
        match self {
            Self::DynamicNew {
                container,
                initializer,
                ..
            } => {
                let target = container_kind(model, *container, Some("dynamic"))?;
                if let Some(source) = initializer {
                    let source = container_kind(model, *source, Some("dynamic"))?;
                    if target.element != source.element {
                        return Err(IrValidationError::new(
                            "container",
                            "dynamic-array initializer element type mismatch",
                        ));
                    }
                }
                Ok(())
            }
            Self::Copy { dst, src } => {
                let dst = container_kind(model, *dst, None)?;
                let src = container_kind(model, *src, None)?;
                let compatible_kind = matches!(
                    (&dst.kind, &src.kind),
                    (IrContainerKind::Dynamic, IrContainerKind::Dynamic)
                        | (IrContainerKind::Queue { .. }, IrContainerKind::Queue { .. })
                ) || matches!(
                    (&dst.kind, &src.kind),
                    (
                        IrContainerKind::Associative { key: dst },
                        IrContainerKind::Associative { key: src }
                    ) if dst == src
                );
                if dst.element != src.element || !compatible_kind {
                    return Err(IrValidationError::new(
                        "container",
                        "container copy type mismatch",
                    ));
                }
                Ok(())
            }
            Self::AssignValues { container, .. } => {
                let container = container_kind(model, *container, None)?;
                if matches!(container.kind, IrContainerKind::Associative { .. }) {
                    return Err(IrValidationError::new(
                        "container",
                        "positional assignment pattern requires a dynamic array or queue",
                    ));
                }
                Ok(())
            }
            Self::Delete(index) => container_kind(model, *index, None).map(|_| ()),
            Self::Set { container, .. } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "string-keyed associative write requires a string expression",
                    ));
                }
                Ok(())
            }
            Self::SetString { container, key, .. } => {
                string_container(model, *container)?;
                key.validate(model, string_return)
            }
            Self::DeleteIndex { container, .. } => {
                let container = container_kind(model, *container, None)?;
                if matches!(container.kind, IrContainerKind::Dynamic) {
                    return Err(IrValidationError::new(
                        "container",
                        "dynamic-array delete method does not take an index",
                    ));
                }
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "string-keyed associative delete requires a string expression",
                    ));
                }
                Ok(())
            }
            Self::DeleteString { container, key } => {
                string_container(model, *container)?;
                key.validate(model, string_return)
            }
            Self::QueuePushFront { container, .. }
            | Self::QueuePushBack { container, .. }
            | Self::QueueInsert { container, .. } => {
                container_kind(model, *container, Some("queue")).map(|_| ())
            }
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::DynamicNew { size, .. } => visit(size),
            Self::Set { index, value, .. } | Self::QueueInsert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            Self::SetString { key, value, .. } => {
                key.expressions(visit);
                visit(value);
            }
            Self::QueuePushFront { value, .. } | Self::QueuePushBack { value, .. } => visit(value),
            Self::DeleteIndex { index, .. } => visit(index),
            Self::DeleteString { key, .. } => key.expressions(visit),
            Self::AssignValues { values, .. } => values.iter().for_each(visit),
            Self::Copy { .. } | Self::Delete(_) => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::DynamicNew { size, .. } => visit(size),
            Self::Set { index, value, .. } | Self::QueueInsert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            Self::SetString { key, value, .. } => {
                key.expressions_mut(visit);
                visit(value);
            }
            Self::QueuePushFront { value, .. } | Self::QueuePushBack { value, .. } => visit(value),
            Self::DeleteIndex { index, .. } => visit(index),
            Self::DeleteString { key, .. } => key.expressions_mut(visit),
            Self::AssignValues { values, .. } => values.iter_mut().for_each(visit),
            Self::Copy { .. } | Self::Delete(_) => {}
        }
    }
}

fn string_container(
    model: &super::IrModel,
    index: usize,
) -> Result<&IrContainer, IrValidationError> {
    let container = container_kind(model, index, Some("associative"))?;
    if !matches!(
        container.kind,
        IrContainerKind::Associative {
            key: IrAssocKey::String
        }
    ) {
        return Err(IrValidationError::new(
            "container",
            "operation requires a string-keyed associative array",
        ));
    }
    Ok(container)
}

fn container_kind<'a>(
    model: &'a super::IrModel,
    index: usize,
    expected: Option<&str>,
) -> Result<&'a IrContainer, IrValidationError> {
    let container = model.containers.get(index).ok_or_else(|| {
        IrValidationError::new("container", format!("index {index} is out of bounds"))
    })?;
    let actual = match container.kind {
        IrContainerKind::Dynamic => "dynamic",
        IrContainerKind::Queue { .. } => "queue",
        IrContainerKind::Associative { .. } => "associative",
    };
    if let Some(expected) = expected.filter(|expected| *expected != actual) {
        return Err(IrValidationError::new(
            "container",
            format!("index {index} refers to {actual}, expected {expected}"),
        ));
    }
    Ok(container)
}
