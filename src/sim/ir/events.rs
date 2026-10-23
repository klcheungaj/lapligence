//! Events.

use super::*;

/// Event-control edge kinds (`LLG_EV_POSEDGE`/`LLG_EV_NEGEDGE`/`LLG_EV_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrEdge {
    Posedge,
    Negedge,
    Any,
}

/// A named-event handle reference. `Static` points at one lowered handle,
/// while `Array` resolves an unpacked event-array element at the point where
/// the operation is issued. `Null` is the legal null event handle and keeps
/// the operation suspended/no-op without manufacturing a pulse object.
#[derive(Clone, Debug, PartialEq)]
pub enum IrEventRef {
    Static(usize),
    Array {
        /// Index of the array descriptor in [`IrModel::events`].
        array: usize,
        indices: Vec<IrExpr>,
    },
    /// A handle copied into activation-owned storage at call time.
    Captured(String),
    /// The activation-owned handle of an input named-event formal of the
    /// enclosing function (`IrFormal::event`, by value).
    Formal(usize),
    /// An event handle stored as an identity element of resizable container
    /// storage (SV 6.17, 7.5-7.10). The handle expression yields the
    /// synchronization object, or null for a missing or invalid element; it
    /// is resolved where the operation is issued, like `Array`.
    Handle(Box<IrChandleExpr>),
    Null,
}

impl IrEventRef {
    /// Visit the integral expressions evaluated to resolve this handle.
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Array { indices, .. } => indices.iter().for_each(visit),
            Self::Handle(handle) => handle.expressions(visit),
            Self::Static(_) | Self::Captured(_) | Self::Formal(_) | Self::Null => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Array { indices, .. } => indices.iter_mut().for_each(visit),
            Self::Handle(handle) => handle.expressions_mut(visit),
            Self::Static(_) | Self::Captured(_) | Self::Formal(_) | Self::Null => {}
        }
    }
}

/// One source entry of an atomic multi-source wait: a signal/array-element
/// wait address (its C name, `&`-prefixed at emission) or a named event
/// handle reference.
#[derive(Clone, Debug, PartialEq)]
pub enum IrWaitSrc {
    /// A value expression, evaluated synchronously when a dependency changes.
    Evaluated {
        eval: String,
        condition: Option<String>,
        reads: Vec<IrDependency>,
    },
    /// A real-valued expression, evaluated synchronously when one of its
    /// typed dependencies changes. Real event controls use bitwise value
    /// change semantics, matching runtime real assignment observation.
    EvaluatedReal {
        eval: String,
        condition: Option<String>,
        reads: Vec<IrDependency>,
    },
    /// Named event with a qualifier evaluated at trigger time.
    FilteredEvent {
        event: IrEventRef,
        condition: String,
    },
    /// Signal (or array-element address) C name; edge per the paired
    /// [`IrEdge`].
    Sig(String),
    /// Real/shortreal signal C name; only `IrEdge::Any` is legal.
    Real(String),
    /// Named event; any trigger wakes the waiter ([`IrEdge`] is ignored,
    /// events are edge-triggered by definition).
    Event(IrEventRef),
    /// The whole variable that `ref` formal `index` of the enclosing typed
    /// subroutine names (see [`IrDependency::RefFormal`]); a real formal
    /// only supports `IrEdge::Any`.
    RefFormal { index: usize, real: bool },
    /// The change marker of the string or class handle that `ref` formal
    /// `index` names (see [`IrDependency::NativeRefFormal`]); only
    /// `IrEdge::Any`.
    NativeRefFormal { index: usize, handle: bool },
    /// The change marker of the fork-shared string or handle the waiting
    /// process names `local` (see [`IrDependency::SharedNative`]); only
    /// `IrEdge::Any`.
    SharedNative { local: String, handle: bool },
}

/// Fork join kinds (`LLG_JOIN`/`LLG_JOIN_NONE`/`LLG_JOIN_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrJoinKind {
    Join,
    None,
    Any,
    /// Compiler-generated independent process (`llg_spawn_detached_with_frame`):
    /// no fork group, so `wait fork`/`disable fork` and the issuer's lifetime
    /// never observe it. Only [`IrStmt::CapturedFork`] uses it, and the
    /// issuing process does not suspend.
    Detached,
}

impl IrJoinKind {
    /// Whether the forking process suspends until its join condition holds.
    pub fn suspends(self) -> bool {
        matches!(self, Self::Join | Self::Any)
    }
}

/// Delay evaluated once in the issuing process, before any suspension.
#[derive(Clone, Debug, PartialEq)]
pub enum IrDelay {
    /// Already converted to design-precision ticks.
    Constant(u64),
    /// Numeric module-unit value, with integral scheduler scaling factors.
    Runtime {
        value: Box<IrExpr>,
        unit_ticks: u64,
        precision_ticks: u64,
    },
}

/// Rise/fall/turn-off propagation delays for one inertial driver, already
/// converted to design-precision ticks.  A single source delay is represented
/// by repeating the same value in all three slots; a two-value source form
/// uses the minimum rise/fall value for turn-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrTransitionDelay {
    pub rise: u64,
    pub fall: u64,
    pub turn_off: u64,
}

impl IrTransitionDelay {
    pub const fn uniform(ticks: u64) -> Self {
        Self {
            rise: ticks,
            fall: ticks,
            turn_off: ticks,
        }
    }
}

/// Propagation delays of one inertial driver site.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInertialDelay {
    /// Constant delays, already in design-precision ticks.
    Constant(IrTransitionDelay),
    /// Delays with a runtime-valued expression (IEEE 1800-2009 28.16.1:
    /// "These can be any three expressions"), evaluated in the driver each
    /// time it schedules an update.
    Runtime(Box<IrRuntimeTransitionDelay>),
    /// The writable delays of SDF-annotated gate `slot`
    /// ([`super::IrModel::sdf_gate_delays`]), read each time the driver
    /// schedules an update.
    Annotated(u32),
}

/// Source delay values of a runtime-valued driver delay. One value serves
/// every transition; with two, turn-off is the smaller of rise and fall, as
/// for [`IrTransitionDelay`].
#[derive(Clone, Debug, PartialEq)]
pub struct IrRuntimeTransitionDelay {
    pub rise: IrDelay,
    pub fall: Option<IrDelay>,
    pub turn_off: Option<IrDelay>,
}

impl IrRuntimeTransitionDelay {
    /// The delay values, in rise, fall, turn-off order.
    pub(in crate::sim) fn values(&self) -> impl Iterator<Item = &IrDelay> {
        [Some(&self.rise), self.fall.as_ref(), self.turn_off.as_ref()]
            .into_iter()
            .flatten()
    }

    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        self.values().filter_map(IrDelay::expression).collect()
    }

    pub(in crate::sim) fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        [
            Some(&mut self.rise),
            self.fall.as_mut(),
            self.turn_off.as_mut(),
        ]
        .into_iter()
        .flatten()
        .filter_map(IrDelay::expression_mut)
        .collect()
    }
}

impl IrInertialDelay {
    /// The constant delays, when no value is runtime-valued.
    pub(in crate::sim) fn constant(&self) -> Option<IrTransitionDelay> {
        match self {
            Self::Constant(delay) => Some(*delay),
            Self::Runtime(_) | Self::Annotated(_) => None,
        }
    }

    /// Runtime delay expressions, in rise, fall, turn-off order.
    pub(in crate::sim) fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Constant(_) | Self::Annotated(_) => Vec::new(),
            Self::Runtime(delay) => delay.expressions(),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        match self {
            Self::Constant(_) | Self::Annotated(_) => Vec::new(),
            Self::Runtime(delay) => delay.expressions_mut(),
        }
    }
}

impl IrDelay {
    pub(in crate::sim) fn expression(&self) -> Option<&IrExpr> {
        match self {
            Self::Constant(_) => None,
            Self::Runtime { value, .. } => Some(value),
        }
    }

    pub(in crate::sim) fn expression_mut(&mut self) -> Option<&mut IrExpr> {
        match self {
            Self::Constant(_) => None,
            Self::Runtime { value, .. } => Some(value),
        }
    }
}

/// One value captured by a fork activation. A synchronous joined branch may
/// borrow its enclosing automatic cell or event handle; detached branches own
/// snapshots, including private handles to the captured event object identity.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCapture {
    pub(super) storage: StorageRef,
    pub(super) initial: IrExpr,
    /// The declaring subroutine's local name for the captured variable when
    /// the branch body reads it under that name (a selected or projected
    /// access, such as an element of an automatic array, inside a nested
    /// fork); the branch binds it to the captured cell too.
    pub(super) alias: Option<String>,
}

/// One automatic value copied into an evaluated-event environment.
///
/// The generated callback uses `local` only as a lexical substitution key;
/// `storage` remains the typed identity used to allocate the runtime frame.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEventCapture {
    pub(super) storage: StorageRef,
    pub(super) local: String,
    pub(super) initial: IrExpr,
}

impl IrEventCapture {
    pub fn new(storage: StorageRef, local: String, initial: IrExpr) -> Self {
        Self {
            storage,
            local,
            initial,
        }
    }

    pub fn storage(&self) -> StorageRef {
        self.storage
    }

    pub fn local(&self) -> &str {
        &self.local
    }

    pub fn initial(&self) -> &IrExpr {
        &self.initial
    }

    pub(in crate::sim) fn initial_mut(&mut self) -> &mut IrExpr {
        &mut self.initial
    }
}

/// Persistent evaluator state for an expression event or trigger-time
/// qualifier. The emitter materializes the frame and the runtime retains it
/// across suspension, cancellation, and nested activations.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEventContext {
    pub(super) frame: FrameId,
    pub(super) captures: Vec<IrEventCapture>,
}

impl IrEventContext {
    pub fn new(frame: FrameId, captures: Vec<IrEventCapture>) -> Self {
        Self { frame, captures }
    }

    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn captures(&self) -> &[IrEventCapture] {
        &self.captures
    }

    pub(in crate::sim) fn captures_mut(&mut self) -> &mut [IrEventCapture] {
        &mut self.captures
    }
}

impl IrCapture {
    pub fn new(storage: StorageRef, initial: IrExpr) -> Self {
        Self {
            storage,
            initial,
            alias: None,
        }
    }

    /// Also bind the captured cell under `alias` in the branch.
    pub fn with_alias(mut self, alias: Option<String>) -> Self {
        self.alias = alias;
        self
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    pub fn storage(&self) -> StorageRef {
        self.storage
    }

    pub fn initial(&self) -> &IrExpr {
        &self.initial
    }

    pub(in crate::sim) fn initial_mut(&mut self) -> &mut IrExpr {
        &mut self.initial
    }
}

/// The callback and owned argument frame for one deferred immediate-assertion
/// action. The action body itself lives on the owning [`IrPreFn`]; keeping the
/// statement-side reference small avoids duplicating the callback tree in the
/// executable IR.
#[derive(Clone, Debug, PartialEq)]
pub struct IrDeferredAction {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) frame: FrameId,
    pub(in crate::sim) captures: Vec<IrCapture>,
}

impl IrDeferredAction {
    pub fn new(c_name: String, frame: FrameId, captures: Vec<IrCapture>) -> Self {
        Self {
            c_name,
            frame,
            captures,
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }

    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn captures(&self) -> &[IrCapture] {
        &self.captures
    }

    pub(in crate::sim) fn captures_mut(&mut self) -> &mut [IrCapture] {
        &mut self.captures
    }
}

/// A fork branch carrying one independently-owned activation frame.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCapturedBranch {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) label: String,
    pub(in crate::sim) frame: FrameId,
    pub(in crate::sim) captures: Vec<IrCapture>,
}

impl IrCapturedBranch {
    pub fn new(c_name: String, label: String, frame: FrameId, captures: Vec<IrCapture>) -> Self {
        Self {
            c_name,
            label,
            frame,
            captures,
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn captures(&self) -> &[IrCapture] {
        &self.captures
    }
}

/// A lowered named event (`event ev;`): a global `llg_event_t` handle backed by
/// a persistent runtime synchronization object. Events are never pruned by the
/// optimizer; handle assignment changes future registrations without moving
/// waiters already attached to the old object.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEvent {
    pub(in crate::sim) c_name: String,
    /// Array descriptors do not own a handle themselves. They name the
    /// pointer table and retain the element event indices for emission.
    pub(in crate::sim) array_dims: Option<Vec<(i32, i32)>>,
    pub(in crate::sim) array_elements: Vec<usize>,
}

impl IrEvent {
    pub fn new(c_name: String) -> Self {
        Self {
            c_name,
            array_dims: None,
            array_elements: Vec::new(),
        }
    }

    pub fn new_array(c_name: String, dims: Vec<(i32, i32)>, elements: Vec<usize>) -> Self {
        Self {
            c_name,
            array_dims: Some(dims),
            array_elements: elements,
        }
    }
    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn is_array(&self) -> bool {
        self.array_dims.is_some()
    }
    pub fn array_dims(&self) -> Option<&[(i32, i32)]> {
        self.array_dims.as_deref()
    }
    pub fn array_elements(&self) -> &[usize] {
        &self.array_elements
    }
}
