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
    /// Fixed untagged overlay. Members share one payload sized to the largest.
    Union {
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

/// `IrContainerElement::Opaque` kind of a built-in `process` class handle.
pub const PROCESS_ELEMENT_KIND: &str = "Process";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrContainerMember {
    pub name: String,
    pub element: Box<IrContainerElement>,
}

impl IrContainerElement {
    /// Exact payload width for fixed integral shapes, checked before frame allocation.
    pub fn fixed_packed_width(&self) -> Option<u32> {
        match self {
            Self::Packed { width, .. } => Some(*width),
            Self::Aggregate { members, .. } => members.iter().try_fold(0u32, |sum, member| {
                sum.checked_add(member.element.fixed_packed_width()?)
            }),
            Self::Union { members, .. } => members.iter().try_fold(0u32, |largest, member| {
                Some(largest.max(member.element.fixed_packed_width()?))
            }),
            Self::FixedArray {
                dimensions,
                element,
            } => {
                dimensions
                    .iter()
                    .try_fold(element.fixed_packed_width()?, |width, (left, right)| {
                        let count = i64::from(*left)
                            .abs_diff(i64::from(*right))
                            .checked_add(1)?;
                        u32::try_from(u64::from(width).checked_mul(count)?).ok()
                    })
            }
            _ => None,
        }
    }

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

    pub fn is_event(&self) -> bool {
        matches!(self, Self::Event)
    }

    /// Handles lowered as ordinary chandle-typed expressions: chandles and
    /// class-like object handles (class, semaphore, mailbox, virtual
    /// interface). Events keep their own handle lowering.
    pub fn is_object_handle(&self) -> bool {
        match self {
            Self::Chandle => true,
            Self::Opaque { kind, .. } => kind != PROCESS_ELEMENT_KIND,
            _ => false,
        }
    }

    /// `process` handles: identities that keep a reference count.
    pub fn is_process(&self) -> bool {
        matches!(self, Self::Opaque { kind, .. } if kind == PROCESS_ELEMENT_KIND)
    }

    /// Elements stored as one pointer-sized identity: borrowed chandles,
    /// event synchronization objects and class-like object handles. Copies
    /// share the referenced object (SV 6.17, 8.4); none is deep-copied.
    pub fn is_handle(&self) -> bool {
        matches!(self, Self::Chandle | Self::Event | Self::Opaque { .. })
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
            (Self::Aggregate { type_id: dst, .. }, Self::Aggregate { type_id: src, .. })
            | (Self::Union { type_id: dst, .. }, Self::Union { type_id: src, .. }) => dst == src,
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
    /// Fixed-size views (currently virtual-interface arrays) use the generic
    /// container runtime so their handle elements can be selected uniformly.
    /// The main initializer allocates this many null/default elements before
    /// any process can assign or read one.
    pub initial_size: Option<u64>,
    /// Per-activation subroutine storage: a formal, result, automatic local
    /// or call temporary. It is created by [`IrContainerStmt::Declare`] (or
    /// bound to a container formal), owned by the enclosing lexical value
    /// scope and has no model-global declaration or change dependencies.
    pub activation: bool,
    /// Instance property storage of class `.0`, field `.1`: one container
    /// per object, reached through the receiver of the enclosing method.
    pub class_field: Option<(usize, usize)>,
    /// The property's container selected through an explicit handle
    /// (`h.q`, SIM-011): an alias of the object's storage, evaluated at each
    /// use, instead of the enclosing method's `this`. Only side-effect-free
    /// handle reads (see [`IrChandleExpr::is_plain_receiver`]) qualify.
    pub receiver: Option<IrChandleExpr>,
}

impl IrContainer {
    /// Whether this container is one model-global variable with change
    /// dependencies (neither activation nor per-object storage).
    pub fn is_global_storage(&self) -> bool {
        !self.activation && self.class_field.is_none()
    }

    /// Whether `other` uses the same runtime storage type: the same container
    /// kind (queue bounds aside), associative key and element shape.
    pub fn same_storage_type(&self, other: &Self) -> bool {
        let kind = match (&self.kind, &other.kind) {
            (IrContainerKind::Dynamic, IrContainerKind::Dynamic)
            | (IrContainerKind::Queue { .. }, IrContainerKind::Queue { .. }) => true,
            (
                IrContainerKind::Associative { key: left },
                IrContainerKind::Associative { key: right },
            ) => left == right,
            _ => false,
        };
        kind && self.element == other.element
    }
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

/// A runtime selector attached to a streaming concatenation operand.  The
/// selector expressions are evaluated by the generated expression/call site
/// exactly once; the runtime then derives the requested logical index range.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStreamSelector {
    /// A single unpacked-array element (`with [index]`).
    Index(IrExpr),
    /// An explicit unpacked-array range (`with [left:right]`).
    Range { left: IrExpr, right: IrExpr },
    /// An indexed range (`with [base +: width]` / `with [base -: width]`).
    Indexed {
        base: IrExpr,
        width: IrExpr,
        negative: bool,
    },
}

/// One operand of a runtime-sized bit stream, in stream order (SV 11.4.14.1).
#[derive(Clone, Debug, PartialEq)]
pub enum IrStreamSegment {
    /// A packed value; a runtime-sized stream value carries its own width.
    Packed(IrExpr),
    /// The packed elements of a dynamic array or queue in index order, or of
    /// a `with` selection whose indices past the end stream the element
    /// default (SV 11.4.14.4); or an associative array's values in index
    /// order (no selector).
    Container {
        container: usize,
        selector: Option<IrStreamSelector>,
    },
    /// The bytes of a string, index 0 leftmost (SV 6.24.3).
    String(IrStringExpr),
    /// A nested streaming concatenation.
    Nested(Box<IrBitStream>),
}

/// A runtime-sized bit stream (SV 6.24.3, 11.4.14) that is never limited by
/// the packed value width: the segments are concatenated, reordered by the
/// pack operator (`slice`, `direction`), and, for a whole unpack into one
/// resizable destination, by the inverse of the target operator (`unpack`).
#[derive(Clone, Debug, PartialEq)]
pub struct IrBitStream {
    pub segments: Vec<IrStreamSegment>,
    pub slice: u32,
    pub direction: super::IrStreamDirection,
    pub unpack: Option<(u32, super::IrStreamDirection)>,
}

impl IrBitStream {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), IrValidationError> {
        if self.slice == 0 || self.unpack.is_some_and(|(slice, _)| slice == 0) {
            return Err(IrValidationError::new(
                "container",
                "bit stream slice size must be positive",
            ));
        }
        for segment in &self.segments {
            match segment {
                IrStreamSegment::Packed(value) => {
                    if value.is_real() || value.fill.is_some() {
                        return Err(IrValidationError::new(
                            "container",
                            "bit stream packed segment must be integral",
                        ));
                    }
                }
                IrStreamSegment::Container {
                    container,
                    selector,
                } => {
                    let container = container_kind(model, *container, None)?;
                    let associative = matches!(container.kind, IrContainerKind::Associative { .. });
                    if !container.element.is_packed() || (associative && selector.is_some()) {
                        return Err(IrValidationError::new(
                            "container",
                            "bit stream container segment requires packed elements",
                        ));
                    }
                    if let Some(selector) = selector {
                        validate_stream_selector(selector)?;
                    }
                }
                IrStreamSegment::String(value) => value.validate(model, string_return)?,
                IrStreamSegment::Nested(stream) => {
                    if stream.unpack.is_some() {
                        return Err(IrValidationError::new(
                            "container",
                            "nested bit stream cannot unpack",
                        ));
                    }
                    stream.validate(model, string_return)?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        for segment in &self.segments {
            match segment {
                IrStreamSegment::Packed(value) => visit(value),
                IrStreamSegment::Container {
                    selector: Some(selector),
                    ..
                } => stream_selector_expressions(selector, visit),
                IrStreamSegment::Container { selector: None, .. } => {}
                IrStreamSegment::String(value) => value.expressions(visit),
                IrStreamSegment::Nested(stream) => stream.expressions(visit),
            }
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        for segment in &mut self.segments {
            match segment {
                IrStreamSegment::Packed(value) => visit(value),
                IrStreamSegment::Container {
                    selector: Some(selector),
                    ..
                } => stream_selector_expressions_mut(selector, visit),
                IrStreamSegment::Container { selector: None, .. } => {}
                IrStreamSegment::String(value) => value.expressions_mut(visit),
                IrStreamSegment::Nested(stream) => stream.expressions_mut(visit),
            }
        }
    }
}

/// Container expressions return packed element values or packed method status.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum IrContainerExpr {
    /// Stream packed elements from a dynamic array or queue in logical index
    /// order.  A runtime selector is optional; the resulting packed width is
    /// dynamic and is represented by the enclosing expression's capacity
    /// width.
    Stream {
        container: usize,
        slice: u32,
        direction: super::IrStreamDirection,
        selector: Option<IrStreamSelector>,
    },
    /// A runtime-sized bit stream materialized as one packed value whose
    /// runtime width is its length; the expression records `LLG_MAX_WIDTH`.
    BitStream(Box<IrBitStream>),
    Size(usize),
    Reduce {
        container: usize,
        operation: IrContainerReduction,
    },
    /// Reduction over the value produced by a typed iterator expression.
    /// `result_*` is the self-determined type of that expression, which may
    /// differ from the source element width.
    ReduceWith {
        container: usize,
        operation: IrContainerReduction,
        callback: String,
        result_width: u32,
        result_signed: bool,
        result_two_state: bool,
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
    /// `size()` of a nested container element selected by integral
    /// indices; a missing element has size 0.
    NestedSize {
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
    /// Whole-array `==`/`!=` (or, with `case`, `===`/`!==`) of two dynamic
    /// arrays or two queues of one element type, including fixed-array
    /// views (SV 7.6, 11.4.5). Elements compare in index order; different
    /// sizes are unequal; the result is one unsigned bit.
    Equal {
        left: usize,
        right: usize,
        case: bool,
        negate: bool,
    },
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

/// Array manipulation methods whose packed-element runtime implementation is
/// shared by dynamic arrays, queues, and supported associative arrays. Methods
/// returning a queue are represented by [`IrContainerStmt::MethodAssign`];
/// in-place methods use [`IrContainerStmt::Method`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrContainerMethod {
    Find,
    FindIndex,
    FindFirst,
    FindFirstIndex,
    FindLast,
    FindLastIndex,
    Min,
    Max,
    Unique,
    UniqueIndex,
    Sort,
    RSort,
    Reverse,
    Shuffle,
}

/// The capture name that stands for shared activation container `index`
/// (a `StorageKind::Container` capture reads it as a `LocalRead`).
pub(in crate::sim) fn shared_container_capture_name(index: usize) -> String {
    format!("_llg_shared_container_{index}")
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrContainerStmt {
    /// Create one empty activation container ([`IrContainer::activation`])
    /// owned by the enclosing lexical value scope.
    Declare(usize),
    /// Like [`IrContainerStmt::Declare`], but the container lives in a shared
    /// reference-counted activation frame that fork branches alias
    /// (SIM-010), so it lives until the declaring scope and every branch
    /// using it are done.
    SharedDeclare(usize),
    /// Unstream a packed value into a packed-element dynamic array or queue.
    /// Selector expressions are retained so the runtime can resize the target
    /// and update the requested logical elements after evaluating them once.
    StreamAssign {
        container: usize,
        source: IrExpr,
        slice: u32,
        direction: super::IrStreamDirection,
        selector: Option<IrStreamSelector>,
    },
    /// Replace a packed-element dynamic array or queue with the elements of
    /// a runtime-sized bit stream. `exact` (a bit-stream cast or a whole
    /// unpack) requires whole elements; otherwise the stream is left-aligned
    /// and zero-filled (SV 6.24.3, 11.4.14, 11.4.14.4).
    BitStreamAssign {
        container: usize,
        stream: IrBitStream,
        exact: bool,
    },
    DynamicNew {
        container: usize,
        size: IrExpr,
        initializer: Option<usize>,
    },
    Copy {
        dst: usize,
        src: usize,
    },
    /// Untimed nonblocking write to persistent descriptor-backed array
    /// storage: the whole of `src` (`dst_start` absent), or `count` source
    /// elements from `src_start` written from storage position `dst_start`.
    /// Source elements and the destination position are captured at issue
    /// (SV 10.4.2).
    Nonblocking {
        target: usize,
        dst_start: Option<IrExpr>,
        src: usize,
        src_start: IrExpr,
        count: u64,
    },
    /// Ambiguous-predicate conditional merge of two descriptor-backed
    /// dynamic arrays into `dst` (SV 11.4.11): known-equal immediate
    /// elements survive, others take their default-uninitialized value.
    Merge {
        dst: usize,
        left: usize,
        right: usize,
    },
    /// Copy `count` elements between fixed-array views of record, string or
    /// handle elements: a slice read (`dst_start` zero) or a slice write.
    /// Starts are signed storage positions evaluated once; out-of-range
    /// source elements read their default and out-of-range destinations
    /// are not written (SV 7.4.6). Sources are snapshotted before writes.
    CopyRange {
        dst: usize,
        dst_start: IrExpr,
        src: usize,
        src_start: IrExpr,
        count: u64,
    },
    /// Assign a queue-valued locator/min/max/unique method result. The source
    /// is evaluated before replacing the destination, so aliasing a
    /// destination with its receiver remains well-defined.
    MethodAssign {
        dst: usize,
        src: usize,
        method: IrContainerMethod,
        callback: Option<String>,
    },
    /// Replace queue `dst` with the elements of `src` (or, with `keys`, their
    /// indices: positions of a dynamic array or queue, keys of an associative
    /// array) at the ordinal positions held in the packed queue `positions`,
    /// in that order (SIM-019). The positions were recorded by a generated
    /// loop that evaluated the method's `with` expression once per element;
    /// positions the receiver no longer holds are skipped.
    Gather {
        dst: usize,
        src: usize,
        positions: usize,
        keys: bool,
    },
    /// Replace the packed position queue `positions` with the ascending
    /// positions of the first element of each distinct key in the parallel
    /// key queue `keys` (packed, real or string), for `unique`.
    UniquePositions {
        positions: usize,
        keys: usize,
    },
    /// Stable sort (or, `descending`, rsort) of a dynamic array or queue by
    /// the parallel key queue `keys` (packed, real or string), computed in
    /// index order before the call (SV 7.12.2).
    SortByKeys {
        container: usize,
        keys: usize,
        descending: bool,
    },
    /// Mutate a dynamic array or queue in place (sort/rsort/reverse/shuffle).
    Method {
        container: usize,
        method: IrContainerMethod,
        callback: Option<String>,
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
    /// Copy a whole record/fixed-array element from a native value root of
    /// the element's type into `slot` (SV 7.5-7.10 value semantics; the root
    /// stays unchanged). Reachable slots: an element, push or insert.
    SetValue {
        container: usize,
        slot: IrValueSlot,
        value: usize,
    },
    /// Copy an element (or the Table 7-1 default for a missing one) into a
    /// native value root; pop slots also remove it.
    GetValue {
        container: usize,
        slot: IrValueSlot,
        value: usize,
    },
    /// Replace `container` with the queue or dynamic-array member at item
    /// path `items` of `root` (SIM-007). A record element of container
    /// storage keeps such a member as a nested dynamic array in its value; a
    /// native record keeps it in a companion container, so a whole-element
    /// read moves it into the temporary's companion, and a member operation
    /// on an element stages it in a lexical container.
    ValueItemToContainer {
        root: IrValueItemRoot,
        items: Vec<u32>,
        container: usize,
    },
    /// Replace the queue or dynamic-array member slot at `items` of `root`
    /// with a deep copy of `container`: before a whole-element write stores
    /// a native value, or after a member operation on an element changed
    /// its staged copy (SIM-007).
    ContainerToValueItem {
        container: usize,
        root: IrValueItemRoot,
        items: Vec<u32>,
    },
}

/// The value whose item a record container member transfer addresses.
#[derive(Clone, Debug, PartialEq)]
pub enum IrValueItemRoot {
    /// A native value root.
    Value(usize),
    /// A record element of container storage, located by an
    /// [`super::IrChandleExpr::ContainerElement`] evaluated at the transfer;
    /// a write locator publishes the container change afterwards.
    Element(super::IrChandleExpr),
}

impl IrValueItemRoot {
    /// The record type of the root value.
    fn element<'a>(
        &self,
        model: &'a super::IrModel,
    ) -> Result<&'a IrContainerElement, IrValidationError> {
        match self {
            Self::Value(value) => {
                let root = model.native_values.get(*value).ok_or_else(|| {
                    IrValidationError::new("container value", "native value is out of bounds")
                })?;
                model.native_types.get(root.ty).ok_or_else(|| {
                    IrValidationError::new("container value", "native type is out of bounds")
                })
            }
            Self::Element(super::IrChandleExpr::ContainerElement {
                container,
                indices,
                key,
                ..
            }) => {
                let storage = model.containers.get(*container).ok_or_else(|| {
                    IrValidationError::new("container element", "container index is out of bounds")
                })?;
                IrValueSlot::Element {
                    indices: indices.clone(),
                    key: key.as_deref().cloned(),
                }
                .element(storage, true, None, model)
            }
            Self::Element(_) => Err(IrValidationError::new(
                "container value",
                "record member transfer root must locate a container element",
            )),
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let Self::Element(element) = self {
            element.expressions(visit);
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let Self::Element(element) = self {
            element.expressions_mut(visit);
        }
    }
}

/// Where a whole-element value transfer reads or writes.
#[derive(Clone, Debug, PartialEq)]
pub enum IrValueSlot {
    /// One element: integral indices through nested containers, or a key of
    /// a string-indexed associative array.
    Element {
        indices: Vec<IrExpr>,
        key: Option<IrStringExpr>,
    },
    PushFront,
    PushBack,
    Insert(IrExpr),
    PopFront,
    PopBack,
}

impl IrValueSlot {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Element { indices, key } => {
                indices.iter().for_each(&mut *visit);
                if let Some(key) = key {
                    key.expressions(visit);
                }
            }
            Self::Insert(index) => visit(index),
            Self::PushFront | Self::PushBack | Self::PopFront | Self::PopBack => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Element { indices, key } => {
                indices.iter_mut().for_each(&mut *visit);
                if let Some(key) = key {
                    key.expressions_mut(visit);
                }
            }
            Self::Insert(index) => visit(index),
            Self::PushFront | Self::PushBack | Self::PopFront | Self::PopBack => {}
        }
    }

    /// Validate the slot against `container` for a write (`SetValue`) or a
    /// read (`GetValue`), returning the element shape it transfers.
    pub(in crate::sim) fn element<'a>(
        &self,
        container: &'a IrContainer,
        write: bool,
        string_return: Option<bool>,
        model: &super::IrModel,
    ) -> Result<&'a IrContainerElement, IrValidationError> {
        let queue = matches!(container.kind, IrContainerKind::Queue { .. });
        let string_keyed = matches!(
            container.kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        );
        let depth = match self {
            Self::Element { indices, key } => {
                if let Some(key) = key {
                    if !string_keyed || !indices.is_empty() {
                        return Err(IrValidationError::new(
                            "container value",
                            "a string key selects one element of a string-keyed array",
                        ));
                    }
                    key.validate(model, string_return)?;
                    1
                } else {
                    if string_keyed || indices.is_empty() || indices.iter().any(IrExpr::is_real) {
                        return Err(IrValidationError::new(
                            "container value",
                            "element indices must be integral and match the key kind",
                        ));
                    }
                    indices.len()
                }
            }
            Self::PushFront | Self::PushBack | Self::Insert(_) if write && queue => {
                if matches!(self, Self::Insert(index) if index.is_real()) {
                    return Err(IrValidationError::new(
                        "container value",
                        "queue insert index must be integral",
                    ));
                }
                1
            }
            Self::PopFront | Self::PopBack if !write && queue => 1,
            _ => {
                return Err(IrValidationError::new(
                    "container value",
                    "queue slot is not valid for this transfer direction or container",
                ))
            }
        };
        let element = nested_element(container, depth).ok_or_else(|| {
            IrValidationError::new("container value", "slot crosses a non-container element")
        })?;
        if !matches!(
            element,
            IrContainerElement::Aggregate { .. } | IrContainerElement::FixedArray { .. }
        ) {
            return Err(IrValidationError::new(
                "container value",
                "whole-value transfers apply to record and fixed-array elements",
            ));
        }
        Ok(element)
    }
}

pub(super) fn validate_stream_selector(
    selector: &IrStreamSelector,
) -> Result<(), IrValidationError> {
    match selector {
        IrStreamSelector::Index(index) => {
            if index.is_real() {
                return Err(IrValidationError::new(
                    "container.selector",
                    "streaming selector index must be integral",
                ));
            }
        }
        IrStreamSelector::Range { left, right } => {
            if left.is_real() || right.is_real() {
                return Err(IrValidationError::new(
                    "container.selector",
                    "streaming selector bounds must be integral",
                ));
            }
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            if base.is_real() || width.is_real() {
                return Err(IrValidationError::new(
                    "container.selector",
                    "indexed streaming selector bounds must be integral",
                ));
            }
        }
    }
    Ok(())
}

fn stream_selector_expressions(selector: &IrStreamSelector, visit: &mut impl FnMut(&IrExpr)) {
    match selector {
        IrStreamSelector::Index(index) => visit(index),
        IrStreamSelector::Range { left, right } => {
            visit(left);
            visit(right);
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            visit(base);
            visit(width);
        }
    }
}

fn stream_selector_expressions_mut(
    selector: &mut IrStreamSelector,
    visit: &mut impl FnMut(&mut IrExpr),
) {
    match selector {
        IrStreamSelector::Index(index) => visit(index),
        IrStreamSelector::Range { left, right } => {
            visit(left);
            visit(right);
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            visit(base);
            visit(width);
        }
    }
}

impl IrContainerExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), IrValidationError> {
        let (index, expected) = match self {
            Self::BitStream(stream) => {
                if stream.unpack.is_some() {
                    return Err(IrValidationError::new(
                        "container",
                        "a bit stream value cannot unpack",
                    ));
                }
                return stream.validate(model, string_return);
            }
            Self::Stream {
                container,
                slice,
                selector,
                ..
            } => {
                let container = container_kind(model, *container, None)?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !container.element.is_packed()
                {
                    return Err(IrValidationError::new(
                        "container",
                        "streaming expression requires a packed dynamic array or queue",
                    ));
                }
                if *slice == 0 {
                    return Err(IrValidationError::new(
                        "container",
                        "streaming expression slice size must be positive",
                    ));
                }
                if let Some(selector) = selector {
                    validate_stream_selector(selector)?;
                }
                return Ok(());
            }
            Self::Size(index) => (*index, None),
            Self::Reduce { container, .. } => (*container, None),
            Self::ReduceWith {
                container,
                callback,
                result_width,
                ..
            } => {
                let container = container_kind(model, *container, None)?;
                if !container.element.is_packed() || callback.is_empty() || *result_width == 0 {
                    return Err(IrValidationError::new(
                        "container",
                        "with-clause reduction requires packed source/result types and a callback",
                    ));
                }
                return Ok(());
            }
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
            Self::NestedSize { container, indices } => {
                let container = container_kind(model, *container, None)?;
                if matches!(
                    container.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                ) || indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !matches!(
                        nested_element(container, indices.len()),
                        Some(IrContainerElement::Container { .. })
                    )
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nested size requires integral indices selecting a container element",
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
            Self::Equal { left, right, .. } => {
                let left = container_kind(model, *left, None)?;
                let right = container_kind(model, *right, None)?;
                let same_kind = matches!(
                    (&left.kind, &right.kind),
                    (IrContainerKind::Dynamic, IrContainerKind::Dynamic)
                        | (IrContainerKind::Queue { .. }, IrContainerKind::Queue { .. })
                );
                if !same_kind
                    || left.element.is_packed() != right.element.is_packed()
                    || !left.element.compatible_with(&right.element)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "container equality requires two dynamic arrays or two queues of one element type",
                    ));
                }
                return Ok(());
            }
        };
        container_kind(model, index, expected).map(|_| ())
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Stream {
                selector: Some(selector),
                ..
            } => stream_selector_expressions(selector, visit),
            Self::Stream { selector: None, .. } => {}
            Self::BitStream(stream) => stream.expressions(visit),
            Self::Get { index, .. }
            | Self::GetReal { index, .. }
            | Self::Exists { key: index, .. } => visit(index),
            Self::GetNested { indices, .. }
            | Self::GetNestedReal { indices, .. }
            | Self::NestedSize { indices, .. } => indices.iter().for_each(visit),
            Self::GetString { key, .. }
            | Self::GetStringReal { key, .. }
            | Self::ExistsString { key, .. } => key.expressions(visit),
            _ => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Stream {
                selector: Some(selector),
                ..
            } => stream_selector_expressions_mut(selector, visit),
            Self::Stream { selector: None, .. } => {}
            Self::BitStream(stream) => stream.expressions_mut(visit),
            Self::Get { index, .. }
            | Self::GetReal { index, .. }
            | Self::Exists { key: index, .. } => visit(index),
            Self::GetNested { indices, .. }
            | Self::GetNestedReal { indices, .. }
            | Self::NestedSize { indices, .. } => indices.iter_mut().for_each(visit),
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
            Self::StreamAssign {
                container,
                source,
                slice,
                selector,
                ..
            } => {
                let container = container_kind(model, *container, None)?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !container.element.is_packed()
                {
                    return Err(IrValidationError::new(
                        "container",
                        "streaming assignment requires a packed dynamic array or queue",
                    ));
                }
                if *slice == 0 || source.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "streaming assignment requires a packed source and positive slice",
                    ));
                }
                if let Some(selector) = selector {
                    validate_stream_selector(selector)?;
                }
                Ok(())
            }
            Self::BitStreamAssign {
                container, stream, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !container.element.is_packed()
                {
                    return Err(IrValidationError::new(
                        "container",
                        "bit stream assignment requires a packed dynamic array or queue",
                    ));
                }
                stream.validate(model, string_return)
            }
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
                // A dynamic array and a queue assign to each other (SV 7.6).
                let compatible_kind = matches!(
                    (&dst.kind, &src.kind),
                    (
                        IrContainerKind::Dynamic | IrContainerKind::Queue { .. },
                        IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                    )
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
            Self::Nonblocking {
                target,
                dst_start,
                src,
                src_start,
                count,
            } => {
                let target = container_kind(model, *target, None)?;
                let src = container_kind(model, *src, None)?;
                if !matches!(target.kind, IrContainerKind::Dynamic)
                    || !matches!(src.kind, IrContainerKind::Dynamic)
                    || target.element.is_packed()
                    || !target.is_global_storage()
                    || !target.element.compatible_with(&src.element)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nonblocking array write requires persistent descriptor-backed dynamic storage and a source of its element type",
                    ));
                }
                if *count == 0
                    || src_start.is_real()
                    || dst_start.as_ref().is_some_and(IrExpr::is_real)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "nonblocking array write needs a positive count and integral positions",
                    ));
                }
                Ok(())
            }
            Self::Merge { dst, left, right } => {
                let dst = container_kind(model, *dst, None)?;
                for source in [left, right] {
                    let source = container_kind(model, *source, None)?;
                    if !matches!(source.kind, IrContainerKind::Dynamic)
                        || !source.element.compatible_with(&dst.element)
                    {
                        return Err(IrValidationError::new(
                            "container",
                            "container merge requires dynamic arrays of one element type",
                        ));
                    }
                }
                if !matches!(dst.kind, IrContainerKind::Dynamic) || dst.element.is_packed() {
                    return Err(IrValidationError::new(
                        "container",
                        "container merge requires a descriptor-backed dynamic array",
                    ));
                }
                Ok(())
            }
            Self::ValueItemToContainer {
                root,
                items,
                container,
            }
            | Self::ContainerToValueItem {
                container,
                root,
                items,
            } => {
                let container = container_kind(model, *container, None)?;
                let mut item = Some(root.element(model)?);
                for step in items {
                    item = match item {
                        Some(IrContainerElement::Aggregate { members, .. }) => members
                            .get(*step as usize)
                            .map(|member| member.element.as_ref()),
                        Some(IrContainerElement::FixedArray { element, .. }) => {
                            Some(element.as_ref())
                        }
                        _ => None,
                    };
                }
                match item {
                    // An empty path names a whole native value of container
                    // type, such as a mailbox message (SIM-017).
                    Some(IrContainerElement::Container { element, .. })
                        if (!items.is_empty() || matches!(root, IrValueItemRoot::Value(_)))
                            && !matches!(container.kind, IrContainerKind::Associative { .. })
                            && element.compatible_with(&container.element)
                            && container.element.compatible_with(element) =>
                    {
                        Ok(())
                    }
                    _ => Err(IrValidationError::new(
                        "container value",
                        "record container member transfer needs a queue or dynamic array of the member's element type",
                    )),
                }
            }
            Self::CopyRange {
                dst,
                dst_start,
                src,
                src_start,
                count,
            } => {
                let dst = container_kind(model, *dst, None)?;
                let src = container_kind(model, *src, None)?;
                if !matches!(dst.kind, IrContainerKind::Dynamic)
                    || !matches!(src.kind, IrContainerKind::Dynamic)
                    || dst.element.is_packed()
                    || !dst.element.compatible_with(&src.element)
                {
                    return Err(IrValidationError::new(
                        "container",
                        "container range copy requires two descriptor-backed dynamic arrays of one element type",
                    ));
                }
                if *count == 0 || dst_start.is_real() || src_start.is_real() {
                    return Err(IrValidationError::new(
                        "container",
                        "container range copy needs a positive count and integral starts",
                    ));
                }
                Ok(())
            }
            Self::MethodAssign {
                dst,
                src,
                method,
                callback,
            } => {
                let destination = container_kind(model, *dst, Some("queue"))?;
                let source = container_kind(model, *src, None)?;
                // Real sources (resizable, unkeyed except for find*) return
                // a real queue, or an integral queue for the *_index forms.
                let index_result = matches!(
                    method,
                    IrContainerMethod::FindIndex
                        | IrContainerMethod::FindFirstIndex
                        | IrContainerMethod::FindLastIndex
                        | IrContainerMethod::UniqueIndex
                );
                let real_source = source.element.is_real()
                    && matches!(
                        source.kind,
                        IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                    )
                    && destination.element.is_packed() == index_result
                    && (index_result || destination.element == source.element)
                    && (callback.is_none()
                        || !matches!(
                            method,
                            IrContainerMethod::Min
                                | IrContainerMethod::Max
                                | IrContainerMethod::Unique
                                | IrContainerMethod::UniqueIndex
                        ));
                if !real_source
                    && (!matches!(
                        source.kind,
                        IrContainerKind::Dynamic
                            | IrContainerKind::Queue { .. }
                            | IrContainerKind::Associative { .. }
                    ) || !destination.element.is_packed()
                        || !source.element.is_packed())
                {
                    return Err(IrValidationError::new(
                        "container",
                        "array method result requires a packed source and queue destination",
                    ));
                }
                if !matches!(
                    method,
                    IrContainerMethod::Find
                        | IrContainerMethod::FindIndex
                        | IrContainerMethod::FindFirst
                        | IrContainerMethod::FindFirstIndex
                        | IrContainerMethod::FindLast
                        | IrContainerMethod::FindLastIndex
                        | IrContainerMethod::Min
                        | IrContainerMethod::Max
                        | IrContainerMethod::Unique
                        | IrContainerMethod::UniqueIndex
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "in-place array method cannot produce a queue result",
                    ));
                }
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
                    return Err(IrValidationError::new(
                        "container",
                        "locator method requires a with-clause callback",
                    ));
                }
                if matches!(
                    source.kind,
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
                    return Err(IrValidationError::new(
                        "container",
                        "packed associative index results require an integral key",
                    ));
                }
                if let Some(callback) = callback {
                    if callback.is_empty() {
                        return Err(IrValidationError::new(
                            "container",
                            "array method callback name is empty",
                        ));
                    }
                }
                Ok(())
            }
            Self::Gather {
                dst,
                src,
                positions,
                keys,
            } => {
                let destination = container_kind(model, *dst, Some("queue"))?;
                let source = container_kind(model, *src, None)?;
                method_positions(model, *positions)?;
                let valid = if *keys {
                    match &source.kind {
                        IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                            destination.element.is_packed()
                        }
                        IrContainerKind::Associative {
                            key: IrAssocKey::Integral { .. },
                        } => destination.element.is_packed(),
                        IrContainerKind::Associative {
                            key: IrAssocKey::String,
                        } => destination.element.is_string(),
                        IrContainerKind::Associative {
                            key: IrAssocKey::Wildcard,
                        } => false,
                    }
                } else {
                    destination.element.is_packed() == source.element.is_packed()
                        && destination.element.compatible_with(&source.element)
                };
                if !valid {
                    return Err(IrValidationError::new(
                        "container",
                        "array-method gather requires a queue of the source element or index type",
                    ));
                }
                Ok(())
            }
            Self::UniquePositions { positions, keys } => {
                method_positions(model, *positions)?;
                method_keys(model, *keys)
            }
            Self::SortByKeys {
                container, keys, ..
            } => {
                let container = container_kind(model, *container, None)?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "keyed ordering requires a dynamic array or queue",
                    ));
                }
                method_keys(model, *keys)
            }
            Self::Method {
                container,
                method,
                callback,
            } => {
                let container = container_kind(model, *container, None)?;
                if !matches!(
                    container.kind,
                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                ) || !(container.element.is_packed()
                    || (callback.is_none()
                        && (matches!(container.element, IrContainerElement::Real { .. })
                            || matches!(
                                method,
                                IrContainerMethod::Reverse | IrContainerMethod::Shuffle
                            ))))
                {
                    return Err(IrValidationError::new(
                        "container",
                        "in-place array method requires a packed dynamic array or queue",
                    ));
                }
                if !matches!(
                    method,
                    IrContainerMethod::Sort
                        | IrContainerMethod::RSort
                        | IrContainerMethod::Reverse
                        | IrContainerMethod::Shuffle
                ) {
                    return Err(IrValidationError::new(
                        "container",
                        "queue-valued array method cannot be used as an in-place method",
                    ));
                }
                if matches!(
                    method,
                    IrContainerMethod::Reverse | IrContainerMethod::Shuffle
                ) && callback.is_some()
                {
                    return Err(IrValidationError::new(
                        "container",
                        "reverse/shuffle do not accept a with-clause callback",
                    ));
                }
                if let Some(callback) = callback {
                    if callback.is_empty() {
                        return Err(IrValidationError::new(
                            "container",
                            "array method callback name is empty",
                        ));
                    }
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
                if !container.element.is_handle() {
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
                if index.is_real() || !container.element.is_handle() {
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
                if !container.element.is_handle() {
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
            Self::Declare(container) | Self::SharedDeclare(container) => {
                if !container_kind(model, *container, None)?.activation {
                    return Err(IrValidationError::new(
                        "container",
                        "only activation containers have lexical declarations",
                    ));
                }
                Ok(())
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
                if !container.element.is_handle() {
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
            Self::SetValue {
                container,
                slot,
                value,
            }
            | Self::GetValue {
                container,
                slot,
                value,
            } => {
                let container = container_kind(model, *container, None)?;
                let element = slot.element(
                    container,
                    matches!(self, Self::SetValue { .. }),
                    string_return,
                    model,
                )?;
                let root = model.native_values.get(*value).ok_or_else(|| {
                    IrValidationError::new("container value", "native value is out of bounds")
                })?;
                if model.native_types.get(root.ty) != Some(element) {
                    return Err(IrValidationError::new(
                        "container value",
                        "native value type differs from the element type",
                    ));
                }
                Ok(())
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
                if !container.element.is_handle() {
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
                if !container.element.is_handle() || index.is_real() {
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
            Self::StreamAssign {
                source, selector, ..
            } => {
                visit(source);
                if let Some(selector) = selector {
                    stream_selector_expressions(selector, visit);
                }
            }
            Self::BitStreamAssign { stream, .. } => stream.expressions(visit),
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
            Self::SetValue { slot, .. } | Self::GetValue { slot, .. } => slot.expressions(visit),
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
            Self::CopyRange {
                dst_start,
                src_start,
                ..
            } => {
                visit(dst_start);
                visit(src_start);
            }
            Self::Nonblocking {
                dst_start,
                src_start,
                ..
            } => {
                if let Some(dst_start) = dst_start {
                    visit(dst_start);
                }
                visit(src_start);
            }
            Self::Copy { .. }
            | Self::Merge { .. }
            | Self::MethodAssign { .. }
            | Self::Gather { .. }
            | Self::UniquePositions { .. }
            | Self::SortByKeys { .. }
            | Self::Method { .. }
            | Self::Delete(_)
            | Self::ResetDefault(_)
            | Self::Declare(_)
            | Self::SharedDeclare(_) => {}
            Self::ValueItemToContainer { root, .. } | Self::ContainerToValueItem { root, .. } => {
                root.expressions(visit)
            }
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::StreamAssign {
                source, selector, ..
            } => {
                visit(source);
                if let Some(selector) = selector {
                    stream_selector_expressions_mut(selector, visit);
                }
            }
            Self::BitStreamAssign { stream, .. } => stream.expressions_mut(visit),
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
            Self::SetValue { slot, .. } | Self::GetValue { slot, .. } => {
                slot.expressions_mut(visit)
            }
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
            Self::CopyRange {
                dst_start,
                src_start,
                ..
            } => {
                visit(dst_start);
                visit(src_start);
            }
            Self::Nonblocking {
                dst_start,
                src_start,
                ..
            } => {
                if let Some(dst_start) = dst_start {
                    visit(dst_start);
                }
                visit(src_start);
            }
            Self::Copy { .. }
            | Self::Merge { .. }
            | Self::MethodAssign { .. }
            | Self::Gather { .. }
            | Self::UniquePositions { .. }
            | Self::SortByKeys { .. }
            | Self::Method { .. }
            | Self::Delete(_)
            | Self::ResetDefault(_)
            | Self::Declare(_)
            | Self::SharedDeclare(_) => {}
            Self::ValueItemToContainer { root, .. } | Self::ContainerToValueItem { root, .. } => {
                root.expressions_mut(visit)
            }
        }
    }
}

/// The packed position queue of a generated array-method loop.
fn method_positions(model: &super::IrModel, index: usize) -> Result<(), IrValidationError> {
    let positions = container_kind(model, index, Some("queue"))?;
    if !positions.element.is_packed() {
        return Err(IrValidationError::new(
            "container",
            "array-method positions require a packed queue",
        ));
    }
    Ok(())
}

/// The per-element key queue of a generated array-method loop.
fn method_keys(model: &super::IrModel, index: usize) -> Result<(), IrValidationError> {
    let keys = container_kind(model, index, Some("queue"))?;
    if !(keys.element.is_packed() || keys.element.is_real() || keys.element.is_string()) {
        return Err(IrValidationError::new(
            "container",
            "array-method keys must be packed, real or string",
        ));
    }
    Ok(())
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
    nested_element(container, depth).is_some_and(IrContainerElement::is_handle)
}

pub(super) fn container_kind<'a>(
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
