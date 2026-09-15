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
    Null,
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
}

/// Fork join kinds (`LLG_JOIN`/`LLG_JOIN_NONE`/`LLG_JOIN_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrJoinKind {
    Join,
    None,
    Any,
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

/// One value copied into a detached fork activation.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCapture {
    pub(super) storage: StorageRef,
    pub(super) initial: IrExpr,
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
        Self { storage, initial }
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
