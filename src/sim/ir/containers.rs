//! Dynamically sized unpacked container storage and operations.

use super::{IrChandleExpr, IrExpr, IrObjectType, IrStringExpr, IrValidationError};

/// Owned value shape used by a resizable container.
///
/// Packed values intentionally remain a distinct, inline `sv4_t` fast path.
/// Every other shape is described recursively so the C runtime can perform
/// clone, default, move, equality, and drop without borrowing frontend data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrContainerElement {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real {
        shortreal: bool,
    },
    String,
    Chandle,
    Event,
    Aggregate {
        type_id: u64,
        members: Vec<IrContainerMember>,
    },
    FixedArray {
        dimensions: Vec<(i32, i32)>,
        element: Box<IrContainerElement>,
    },
    Container {
        type_id: u64,
        kind: String,
        element: Box<IrContainerElement>,
    },
    Opaque {
        type_id: u64,
        kind: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrContainerMember {
    pub name: String,
    pub element: Box<IrContainerElement>,
}

impl IrContainerElement {
    pub fn packed(&self) -> Option<(u32, bool, bool)> {
        match self {
            Self::Packed {
                width,
                signed,
                two_state,
            } => Some((*width, *signed, *two_state)),
            _ => None,
        }
    }

    pub fn width(&self) -> u32 {
        self.packed().map(|(width, _, _)| width).unwrap_or(0)
    }

    pub fn signed(&self) -> bool {
        self.packed().map(|(_, signed, _)| signed).unwrap_or(false)
    }

    pub fn two_state(&self) -> bool {
        self.packed()
            .map(|(_, _, two_state)| two_state)
            .unwrap_or(false)
    }

    pub fn is_packed(&self) -> bool {
        matches!(self, Self::Packed { .. })
    }

    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real { .. })
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    pub fn is_chandle(&self) -> bool {
        matches!(self, Self::Chandle)
    }

    /// Assignment compatibility for container element values. Integral
    /// packed values use the normal assignment conversion at the runtime;
    /// nominal recursive values retain their frontend type identity.
    pub fn compatible_with(&self, source: &Self) -> bool {
        match (self, source) {
            (Self::Packed { .. }, Self::Packed { .. })
            | (Self::Real { .. }, Self::Real { .. })
            | (Self::String, Self::String)
            | (Self::Chandle, Self::Chandle)
            | (Self::Event, Self::Event) => true,
            (Self::Aggregate { type_id: dst, .. }, Self::Aggregate { type_id: src, .. }) => {
                dst == src
            }
            (
                Self::FixedArray {
                    dimensions: dst_dims,
                    element: dst,
                },
                Self::FixedArray {
                    dimensions: src_dims,
                    element: src,
                },
            ) => dst_dims == src_dims && dst.compatible_with(src),
            (
                Self::Container {
                    type_id: dst_id,
                    kind: dst_kind,
                    element: dst,
                },
                Self::Container {
                    type_id: src_id,
                    kind: src_kind,
                    element: src,
                },
            ) => {
                (dst_id == src_id || (dst_kind == src_kind && dst.compatible_with(src)))
                    && dst.compatible_with(src)
            }
            (
                Self::Opaque {
                    type_id: dst_id,
                    kind: dst_kind,
                },
                Self::Opaque {
                    type_id: src_id,
                    kind: src_kind,
                },
            ) => dst_id == src_id && dst_kind == src_kind,
            _ => false,
        }
    }
}

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
    pub element: IrContainerElement,
    pub kind: IrContainerKind,
}

/// One bound of a queue slice. `$` is kept distinct from an ordinary
/// expression because it denotes the current queue end at evaluation time.
#[derive(Clone, Debug, PartialEq)]
pub enum IrQueueBound {
    Value(IrExpr),
    Unbounded,
}

/// A queue-valued source used by whole-queue assignment. Sources remain views
/// until emission so overlapping slices are materialized by the runtime before
/// replacing the destination.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum IrQueueSource {
    Whole(usize),
    Slice {
        container: usize,
        left: IrQueueBound,
        right: IrQueueBound,
    },
}

/// Container expressions return packed element values or packed method status.
#[derive(Clone, Debug, PartialEq)]
pub enum IrContainerExpr {
    Size(usize),
    Reduce {
        container: usize,
        operation: IrContainerReduction,
    },
    Get {
        container: usize,
        index: Box<IrExpr>,
    },
    /// Real-valued element read from a generic dynamic array.
    GetReal {
        container: usize,
        index: Box<IrExpr>,
    },
    GetNested {
        container: usize,
        indices: Vec<IrExpr>,
    },
    GetNestedReal {
        container: usize,
        indices: Vec<IrExpr>,
    },
    GetString {
        container: usize,
        key: IrStringExpr,
    },
    GetStringReal {
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
    /// String-key traversal using an automatic native-string local rather
    /// than model-global object storage.
    AssocTraverseStringLocal {
        container: usize,
        direction: IrAssocTraversal,
        key_name: String,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrContainerReduction {
    Sum,
    Product,
    BitAnd,
    BitOr,
    BitXor,
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
    QueueAssign {
        container: usize,
        sources: Vec<IrQueueSource>,
    },
    AssignRealValues {
        container: usize,
        values: Vec<IrExpr>,
    },
    AssignStringValues {
        container: usize,
        values: Vec<IrStringExpr>,
    },
    AssignChandleValues {
        container: usize,
        values: Vec<IrChandleExpr>,
    },
    Delete(usize),
    Set {
        container: usize,
        index: IrExpr,
        value: IrExpr,
    },
    SetReal {
        container: usize,
        index: IrExpr,
        value: IrExpr,
    },
    SetStringValue {
        container: usize,
        index: IrExpr,
        value: IrStringExpr,
    },
    SetChandleValue {
        container: usize,
        index: IrExpr,
        value: IrChandleExpr,
    },
    SetNested {
        container: usize,
        indices: Vec<IrExpr>,
        value: IrExpr,
    },
    SetNestedReal {
        container: usize,
        indices: Vec<IrExpr>,
        value: IrExpr,
    },
    SetNestedString {
        container: usize,
        indices: Vec<IrExpr>,
        value: IrStringExpr,
    },
    SetNestedChandle {
        container: usize,
        indices: Vec<IrExpr>,
        value: IrChandleExpr,
    },
    SetContainer {
        container: usize,
        indices: Vec<IrExpr>,
        source: usize,
    },
    /// Set the value returned for a missing associative-array key without
    /// creating an entry.  The default is separate from the array's entries,
    /// so `exists`/`num` remain unchanged.
    SetDefault {
        container: usize,
        value: IrExpr,
    },
    SetDefaultString {
        container: usize,
        value: IrStringExpr,
    },
    SetDefaultChandle {
        container: usize,
        value: IrChandleExpr,
    },
    /// Restore the element-type default used for missing associative keys.
    ResetDefault(usize),
    SetString {
        container: usize,
        key: IrStringExpr,
        value: IrExpr,
    },
    SetStringReal {
        container: usize,
        key: IrStringExpr,
        value: IrExpr,
    },
    SetStringString {
        container: usize,
        key: IrStringExpr,
        value: IrStringExpr,
    },
    SetStringChandle {
        container: usize,
        key: IrStringExpr,
        value: IrChandleExpr,
    },
    QueuePushFront {
        container: usize,
        value: IrExpr,
    },
    QueuePushBack {
        container: usize,
        value: IrExpr,
    },
    QueuePushFrontString {
        container: usize,
        value: IrStringExpr,
    },
    QueuePushBackString {
        container: usize,
        value: IrStringExpr,
    },
    QueuePushFrontChandle {
        container: usize,
        value: IrChandleExpr,
    },
    QueuePushBackChandle {
        container: usize,
        value: IrChandleExpr,
    },
    QueuePushFrontContainer {
        container: usize,
        source: usize,
    },
    QueuePushBackContainer {
        container: usize,
        source: usize,
    },
    QueueInsert {
        container: usize,
        index: IrExpr,
        value: IrExpr,
    },
    QueueInsertString {
        container: usize,
        index: IrExpr,
        value: IrStringExpr,
    },
    QueueInsertChandle {
        container: usize,
        index: IrExpr,
        value: IrChandleExpr,
    },
    QueueInsertContainer {
        container: usize,
        index: IrExpr,
        source: usize,
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
        let (index, expected) = match self {
            Self::Size(index) => (*index, None),
            Self::Reduce { container, .. } => (*container, None),
            Self::Get { container, index } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "container index must be integral",
                    ));
                }
                if !container.element.is_packed() {
                    return Err(IrValidationError::new(
                        "container",
                        "packed container read requires a packed element type",
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
                        "string-keyed associative read requires a string expression",
                    ));
                }
                return Ok(());
            }
            Self::GetReal { container, index } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() || !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "real container read requires an integral index and real element type",
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
                        "real string-keyed associative read requires a string key expression",
                    ));
                }
                return Ok(());
            }
            Self::GetNested { container, indices } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested packed read requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_packed_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested packed read has an invalid index path or element type",
                    ));
                }
                return Ok(());
            }
            Self::GetNestedReal { container, indices } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested real read requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_real_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested real read has an invalid index path or element type",
                    ));
                }
                return Ok(());
            }
            Self::GetString { container, key } => {
                let container = string_container(model, *container)?;
                if !container.element.is_packed() {
                    return Err(IrValidationError::new(
                        "container",
                        "packed string-keyed associative read requires a packed element type",
                    ));
                }
                key.validate(model, string_return)?;
                return Ok(());
            }
            Self::GetStringReal { container, key } => {
                let container = string_container(model, *container)?;
                if !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "real string-keyed associative read requires a real element type",
                    ));
                }
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
            Self::AssocTraverseStringLocal {
                container,
                key_name,
                ..
            } => {
                string_container(model, *container)?;
                if key_name.is_empty() {
                    return Err(IrValidationError::new(
                        "container",
                        "string associative traversal local name is empty",
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
            Self::Get { index, .. }
            | Self::GetReal { index, .. }
            | Self::Exists { key: index, .. } => visit(index),
            Self::GetNested { indices, .. } | Self::GetNestedReal { indices, .. } => {
                indices.iter().for_each(visit)
            }
            Self::GetString { key, .. }
            | Self::GetStringReal { key, .. }
            | Self::ExistsString { key, .. } => key.expressions(visit),
            _ => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Get { index, .. }
            | Self::GetReal { index, .. }
            | Self::Exists { key: index, .. } => visit(index),
            Self::GetNested { indices, .. } | Self::GetNestedReal { indices, .. } => {
                indices.iter_mut().for_each(visit)
            }
            Self::GetString { key, .. }
            | Self::GetStringReal { key, .. }
            | Self::ExistsString { key, .. } => key.expressions_mut(visit),
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
        match self {
            Self::DynamicNew {
                container,
                size,
                initializer,
                ..
            } => {
                let target = container_kind(model, *container, Some("dynamic"))?;
                if size.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "dynamic-array size must be integral",
                    ));
                }
                if let Some(source) = initializer {
                    let source = container_kind(model, *source, Some("dynamic"))?;
                    if !target.element.compatible_with(&source.element) {
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
                if !dst.element.compatible_with(&src.element) || !compatible_kind {
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
            Self::QueueAssign { container, sources } => {
                let destination = container_kind(model, *container, Some("queue"))?;
                if sources.is_empty() {
                    return Err(IrValidationError::new(
                        "container",
                        "queue assignment requires at least one source",
                    ));
                }
                for source in sources {
                    let (source_index, bounds) = match source {
                        IrQueueSource::Whole(source) => (*source, None),
                        IrQueueSource::Slice {
                            container,
                            left,
                            right,
                        } => (*container, Some((left, right))),
                    };
                    let source = container_kind(model, source_index, Some("queue"))?;
                    if !destination.element.compatible_with(&source.element) {
                        return Err(IrValidationError::new(
                            "container",
                            "queue assignment source element type mismatch",
                        ));
                    }
                    if let Some((left, right)) = bounds {
                        for bound in [left, right] {
                            if let IrQueueBound::Value(value) = bound {
                                if value.is_real() {
                                    return Err(IrValidationError::new(
                                        "container",
                                        "queue slice bound must be integral",
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            Self::AssignRealValues { container, values } => {
                let container = container_kind(model, *container, None)?;
                if matches!(container.kind, IrContainerKind::Associative { .. }) {
                    return Err(IrValidationError::new(
                        "container",
                        "real value assignment requires a dynamic array or queue",
                    ));
                }
                if !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "real value assignment requires a real container element type",
                    ));
                }
                if values
                    .iter()
                    .any(|value| !value.is_real() && value.width() == 0)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "real value assignment has an invalid expression",
                    ));
                }
                Ok(())
            }
            Self::AssignStringValues { container, values } => {
                let container = container_kind(model, *container, None)?;
                if matches!(container.kind, IrContainerKind::Associative { .. }) {
                    return Err(IrValidationError::new(
                        "container",
                        "string value assignment requires a dynamic array or queue",
                    ));
                }
                if !container.element.is_string() {
                    return Err(IrValidationError::new(
                        "container",
                        "string value assignment requires a string container element type",
                    ));
                }
                values
                    .iter()
                    .try_for_each(|value| value.validate(model, string_return))
            }
            Self::AssignChandleValues { container, .. } => {
                let container = container_kind(model, *container, None)?;
                if matches!(container.kind, IrContainerKind::Associative { .. }) {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle value assignment requires a dynamic array or queue",
                    ));
                }
                if !container.element.is_chandle() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle value assignment requires a chandle container element type",
                    ));
                }
                Ok(())
            }
            Self::Delete(index) => container_kind(model, *index, None).map(|_| ()),
            Self::Set {
                container, index, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() || !container.element.is_packed() {
                    return Err(IrValidationError::new(
                        "container",
                        "packed container write requires an integral index and packed element type",
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
                        "string-keyed associative write requires a string expression",
                    ));
                }
                Ok(())
            }
            Self::SetReal {
                container, index, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() || !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "real container write requires an integral index and real element type",
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
                        "real string-keyed associative write requires a string key expression",
                    ));
                }
                Ok(())
            }
            Self::SetStringValue {
                container,
                index,
                value,
            } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() || !container.element.is_string() {
                    return Err(IrValidationError::new(
                        "container",
                        "string container write requires an integral index and string element type",
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
                        "string string-keyed associative write requires a string key expression",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::SetChandleValue {
                container, index, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if index.is_real() || !container.element.is_chandle() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle container write requires an integral index and chandle element type",
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
                        "chandle string-keyed associative write requires a string key expression",
                    ));
                }
                Ok(())
            }
            Self::SetNested {
                container, indices, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested packed write requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_packed_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested packed write has an invalid index path or element type",
                    ));
                }
                Ok(())
            }
            Self::SetNestedReal {
                container, indices, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested real write requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_real_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested real write has an invalid index path or element type",
                    ));
                }
                Ok(())
            }
            Self::SetNestedString {
                container,
                indices,
                value,
            } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested string write requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_string_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested string write has an invalid index path or element type",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::SetNestedChandle {
                container, indices, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested chandle write requires a dynamic array or queue",
                    ));
                }
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !nested_chandle_element(container, indices.len())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested chandle write has an invalid index path or element type",
                    ));
                }
                Ok(())
            }
            Self::SetContainer {
                container,
                indices,
                source,
            } => {
                let target = container_kind(model, *container, None)?;
                if matches!(
                    target.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "nested container write requires a dynamic array or queue",
                    ));
                }
                let source = container_kind(model, *source, Some("dynamic"))?;
                let target_element = nested_element(target, indices.len());
                let compatible = matches!(
                    target_element,
                    Some(IrContainerElement::Container { element, .. })
                        if element.compatible_with(&source.element)
                );
                if indices.is_empty() || indices.iter().any(IrExpr::is_real) || !compatible {
                    return Err(IrValidationError::new(
                        "container",
                        "nested container write has an incompatible source",
                    ));
                }
                Ok(())
            }
            Self::SetDefault { container, .. } => {
                let container = container_kind(model, *container, Some("associative"))?;
                if !matches!(container.kind, IrContainerKind::Associative { .. }) {
                    return Err(IrValidationError::new(
                        "container",
                        "associative default requires an associative array",
                    ));
                }
                Ok(())
            }
            Self::SetDefaultString { container, value } => {
                let container = container_kind(model, *container, Some("associative"))?;
                if !container.element.is_string() {
                    return Err(IrValidationError::new(
                        "container",
                        "string associative default requires a string element type",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::SetDefaultChandle { container, .. } => {
                let container = container_kind(model, *container, Some("associative"))?;
                if !container.element.is_chandle() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle associative default requires a chandle element type",
                    ));
                }
                Ok(())
            }
            Self::ResetDefault(container) => {
                container_kind(model, *container, Some("associative")).map(|_| ())
            }
            Self::SetString { container, key, .. } => {
                let container = string_container(model, *container)?;
                if !container.element.is_packed() {
                    return Err(IrValidationError::new(
                        "container",
                        "packed string-keyed associative write requires a packed element type",
                    ));
                }
                key.validate(model, string_return)
            }
            Self::SetStringReal {
                container,
                key,
                value,
            } => {
                let container = string_container(model, *container)?;
                if !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "real string-keyed associative write requires a real element type",
                    ));
                }
                let _ = value;
                key.validate(model, string_return)
            }
            Self::SetStringString {
                container,
                key,
                value,
            } => {
                let container = string_container(model, *container)?;
                if !container.element.is_string() {
                    return Err(IrValidationError::new(
                        "container",
                        "string string-keyed associative write requires a string element type",
                    ));
                }
                key.validate(model, string_return)?;
                value.validate(model, string_return)
            }
            Self::SetStringChandle { container, key, .. } => {
                let container = string_container(model, *container)?;
                if !container.element.is_chandle() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle string-keyed associative write requires a chandle element type",
                    ));
                }
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
                let container = container_kind(model, *container, Some("queue"))?;
                if !container.element.is_packed() && !container.element.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "packed or real queue operation requires a scalar element type",
                    ));
                }
                Ok(())
            }
            Self::QueuePushFrontString { container, value }
            | Self::QueuePushBackString { container, value } => {
                let container = container_kind(model, *container, Some("queue"))?;
                if !container.element.is_string() {
                    return Err(IrValidationError::new(
                        "container",
                        "string queue operation requires a string element type",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::QueuePushFrontChandle { container, .. }
            | Self::QueuePushBackChandle { container, .. } => {
                let container = container_kind(model, *container, Some("queue"))?;
                if !container.element.is_chandle() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle queue operation requires a chandle element type",
                    ));
                }
                Ok(())
            }
            Self::QueuePushFrontContainer { container, source }
            | Self::QueuePushBackContainer { container, source } => {
                let container = container_kind(model, *container, Some("queue"))?;
                let source = container_kind(model, *source, Some("dynamic"))?;
                if !matches!(
                    container.element,
                    IrContainerElement::Container { ref element, .. }
                        if element.compatible_with(&source.element)
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "recursive queue operation has an incompatible source container",
                    ));
                }
                Ok(())
            }
            Self::QueueInsertString {
                container,
                index,
                value,
            } => {
                let container = container_kind(model, *container, Some("queue"))?;
                if !container.element.is_string() || index.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "string queue insertion requires an integral index and string element",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::QueueInsertChandle {
                container, index, ..
            } => {
                let container = container_kind(model, *container, Some("queue"))?;
                if !container.element.is_chandle() || index.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "chandle queue insertion requires an integral index and chandle element",
                    ));
                }
                Ok(())
            }
            Self::QueueInsertContainer {
                container,
                index,
                source,
            } => {
                let container = container_kind(model, *container, Some("queue"))?;
                let source = container_kind(model, *source, Some("dynamic"))?;
                if index.is_real()
                    || !matches!(
                        container.element,
                        IrContainerElement::Container { ref element, .. }
                            if element.compatible_with(&source.element)
                    )
                {
                    return Err(IrValidationError::new(
                        "container",
                        "recursive queue insertion has an invalid index or source container",
                    ));
                }
                Ok(())
            }
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::DynamicNew { size, .. } => visit(size),
            Self::Set { index, value, .. }
            | Self::SetReal { index, value, .. }
            | Self::QueueInsert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            Self::SetStringValue { index, value, .. } => {
                visit(index);
                value.expressions(visit);
            }
            Self::SetChandleValue { index, .. } => visit(index),
            Self::SetNested { indices, value, .. } | Self::SetNestedReal { indices, value, .. } => {
                indices.iter().for_each(&mut *visit);
                visit(value);
            }
            Self::SetNestedString { indices, value, .. } => {
                indices.iter().for_each(&mut *visit);
                value.expressions(visit);
            }
            Self::SetNestedChandle { indices, .. } => indices.iter().for_each(&mut *visit),
            Self::SetContainer { indices, .. } => indices.iter().for_each(&mut *visit),
            Self::SetDefault { value, .. } => visit(value),
            Self::SetDefaultString { value, .. } => value.expressions(visit),
            Self::SetDefaultChandle { .. } => {}
            Self::SetString { key, value, .. } | Self::SetStringReal { key, value, .. } => {
                key.expressions(visit);
                visit(value);
            }
            Self::SetStringString { key, value, .. } => {
                key.expressions(visit);
                value.expressions(visit);
            }
            Self::SetStringChandle { key, .. } => key.expressions(visit),
            Self::QueuePushFront { value, .. } | Self::QueuePushBack { value, .. } => visit(value),
            Self::QueuePushFrontString { value, .. } | Self::QueuePushBackString { value, .. } => {
                value.expressions(visit)
            }
            Self::QueuePushFrontChandle { .. } | Self::QueuePushBackChandle { .. } => {}
            Self::QueuePushFrontContainer { .. } | Self::QueuePushBackContainer { .. } => {}
            Self::QueueInsertString { index, value, .. } => {
                visit(index);
                value.expressions(visit);
            }
            Self::QueueInsertChandle { index, .. } => visit(index),
            Self::QueueInsertContainer { index, .. } => visit(index),
            Self::DeleteIndex { index, .. } => visit(index),
            Self::DeleteString { key, .. } => key.expressions(visit),
            Self::AssignValues { values, .. } | Self::AssignRealValues { values, .. } => {
                values.iter().for_each(visit)
            }
            Self::QueueAssign { sources, .. } => {
                for source in sources {
                    if let IrQueueSource::Slice { left, right, .. } = source {
                        for bound in [left, right] {
                            if let IrQueueBound::Value(value) = bound {
                                visit(value);
                            }
                        }
                    }
                }
            }
            Self::AssignStringValues { values, .. } => {
                values.iter().for_each(|value| value.expressions(visit))
            }
            Self::AssignChandleValues { values, .. } => {
                values.iter().for_each(|value| value.expressions(visit))
            }
            Self::Copy { .. } | Self::Delete(_) | Self::ResetDefault(_) => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::DynamicNew { size, .. } => visit(size),
            Self::Set { index, value, .. }
            | Self::SetReal { index, value, .. }
            | Self::QueueInsert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            Self::SetStringValue { index, value, .. } => {
                visit(index);
                value.expressions_mut(visit);
            }
            Self::SetChandleValue { index, .. } => visit(index),
            Self::SetNested { indices, value, .. } | Self::SetNestedReal { indices, value, .. } => {
                indices.iter_mut().for_each(&mut *visit);
                visit(value);
            }
            Self::SetNestedString { indices, value, .. } => {
                indices.iter_mut().for_each(&mut *visit);
                value.expressions_mut(visit);
            }
            Self::SetNestedChandle { indices, .. } => indices.iter_mut().for_each(&mut *visit),
            Self::SetContainer { indices, .. } => indices.iter_mut().for_each(&mut *visit),
            Self::SetDefault { value, .. } => visit(value),
            Self::SetDefaultString { value, .. } => value.expressions_mut(visit),
            Self::SetDefaultChandle { .. } => {}
            Self::SetString { key, value, .. } | Self::SetStringReal { key, value, .. } => {
                key.expressions_mut(visit);
                visit(value);
            }
            Self::SetStringString { key, value, .. } => {
                key.expressions_mut(visit);
                value.expressions_mut(visit);
            }
            Self::SetStringChandle { key, .. } => key.expressions_mut(visit),
            Self::QueuePushFront { value, .. } | Self::QueuePushBack { value, .. } => visit(value),
            Self::QueuePushFrontString { value, .. } | Self::QueuePushBackString { value, .. } => {
                value.expressions_mut(visit)
            }
            Self::QueuePushFrontChandle { .. } | Self::QueuePushBackChandle { .. } => {}
            Self::QueuePushFrontContainer { .. } | Self::QueuePushBackContainer { .. } => {}
            Self::QueueInsertString { index, value, .. } => {
                visit(index);
                value.expressions_mut(visit);
            }
            Self::QueueInsertChandle { index, .. } => visit(index),
            Self::QueueInsertContainer { index, .. } => visit(index),
            Self::DeleteIndex { index, .. } => visit(index),
            Self::DeleteString { key, .. } => key.expressions_mut(visit),
            Self::AssignValues { values, .. } | Self::AssignRealValues { values, .. } => {
                values.iter_mut().for_each(visit)
            }
            Self::QueueAssign { sources, .. } => {
                for source in sources {
                    if let IrQueueSource::Slice { left, right, .. } = source {
                        for bound in [left, right] {
                            if let IrQueueBound::Value(value) = bound {
                                visit(value);
                            }
                        }
                    }
                }
            }
            Self::AssignStringValues { values, .. } => values
                .iter_mut()
                .for_each(|value| value.expressions_mut(visit)),
            Self::AssignChandleValues { values, .. } => values
                .iter_mut()
                .for_each(|value| value.expressions_mut(visit)),
            Self::Copy { .. } | Self::Delete(_) | Self::ResetDefault(_) => {}
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

fn nested_element(container: &IrContainer, depth: usize) -> Option<&IrContainerElement> {
    let mut element = &container.element;
    for _ in 1..depth {
        let IrContainerElement::Container { element: next, .. } = element else {
            return None;
        };
        element = next;
    }
    Some(element)
}

fn nested_packed_element(container: &IrContainer, depth: usize) -> bool {
    nested_element(container, depth).is_some_and(IrContainerElement::is_packed)
}

fn nested_real_element(container: &IrContainer, depth: usize) -> bool {
    nested_element(container, depth).is_some_and(IrContainerElement::is_real)
}

pub(super) fn nested_string_element(container: &IrContainer, depth: usize) -> bool {
    nested_element(container, depth).is_some_and(IrContainerElement::is_string)
}

pub(super) fn nested_chandle_element(container: &IrContainer, depth: usize) -> bool {
    nested_element(container, depth).is_some_and(IrContainerElement::is_chandle)
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
