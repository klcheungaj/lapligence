//! Calls.

use super::*;

/// Structural call arguments shared by statement-position and
/// expression-position calls.  Input arguments arrive already converted to
/// the formal's width/signedness (defaults substituted at lowering).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCallArg {
    /// Fixed descriptor operand; passing mode belongs to the callee formal.
    FixedArray(usize),
    FixedValue(Box<IrFixedValue>),
    /// Caller-owned native value passed by address to a native-value formal.
    /// Inputs receive a fresh caller copy; outputs and results are written
    /// in place and copied back by the caller after return.
    NativeValue(usize),
    /// A native record `ref` formal that the called specialization binds to
    /// declaration-owned record storage (SIM-008): the callee reads and
    /// writes the record's leaves directly, so the call passes no value
    /// (its parameters are null). Valid only for a formal listed in the
    /// callee's [`super::IrFunc::bound_native_refs`].
    NativeRefBound,
    /// Whole caller container passed to a container formal. The call gives
    /// the callee fresh storage: inputs and inouts copy the actual in,
    /// outputs and results are copied back after the callee returns.
    Container(usize),
    /// Input container formal built at the call from element values evaluated
    /// in order (an assignment pattern in expression position). `container`
    /// is an activation container of the formal's storage type that names
    /// the element type; the call creates its storage.
    ContainerValues {
        container: usize,
        values: Vec<IrExpr>,
    },
    /// Input native-value formal of type `ty` (index into the model's native
    /// types) built from leaf values evaluated in order at the call; leaves
    /// not listed keep their typed default.
    NativeLeaves {
        ty: usize,
        leaves: Vec<super::IrNativeLeafValue>,
        /// Source containers copied into the callee's companion containers,
        /// in companion order (SIM-007); empty when the type has none.
        containers: Vec<usize>,
    },
    /// Input native-value formal fed by a native-result call evaluated at
    /// this operand. `value` is the lexical result storage the inner call
    /// writes through its trailing `NativeValue(value)` operand; it is
    /// declared by this operand and handed to the outer callee.
    NativeCall {
        value: usize,
        call: Box<IrCall>,
    },
    /// Whole real fixed-array storage (index into the model's arrays) for a
    /// real-array formal. Inputs and outputs use a fresh call-owned copy
    /// (outputs publish back after return); a `ref` passes the cells.
    RealArray(usize),
    /// Input real-array formal built from declaration-order element values
    /// evaluated in order at the call (patterns, conditionals, selections).
    RealArrayValues(Vec<IrExpr>),
    /// Input real-array formal fed by a real-array-result call evaluated at
    /// this operand. `array` is the lexical result storage the inner call
    /// writes through its trailing `RealArray(array)` operand; it is declared
    /// by this operand and handed to the outer callee.
    RealArrayCall {
        array: usize,
        call: Box<IrCall>,
    },
    /// Input formal value, captured once before the next input is evaluated.
    /// Later defaults can read its call-local binding from `call_argument_name`.
    Val(IrExpr),
    /// Input native string formal. String bytes stay owned and typed.
    StringVal(IrStringExpr),
    /// Input chandle formal value.  Chandles remain native opaque pointers;
    /// they are never reinterpreted as packed storage.
    ChandleVal(IrChandleExpr),
    /// Input named-event formal. The callee receives the event object the
    /// handle names at the call (a null handle stays null), never the
    /// caller's handle storage, so a later rebinding of the actual cannot
    /// move the callee's waits.
    EventVal(IrEventRef),
    /// Output, inout or `ref` named-event formal bound to the caller's
    /// handle storage (`llg_event_t*`). A `ref` callee rebinds that handle
    /// directly; outputs and inouts work on their own handle and assign it
    /// back when the callee returns.
    EventAddr(IrEventRef),
    /// `ref` named-event formal bound to a class event property. The slot
    /// holds the object identity, the one pointer an `llg_event_t` holds, so
    /// a rebinding by the callee reaches the property at once (SV 13.5.2).
    EventSlot(String),
    /// Output/inout chandle formal bound to a caller-owned `void **`.
    ChandleAddr(String),
    /// Chandle `ref` formal bound to a caller-owned pointer slot.
    ChandleRefAddr(String),
    /// Output/inout native string formal bound to a caller-owned slot.
    StringOutAddr(String),
    /// Native string `ref` formal bound to a whole caller-owned slot.
    /// `observed`: the callee waits on the formal, so the call first gives a
    /// class string property its change marker
    /// (`llg_class_string_dependency`); other storage carries its own.
    StringRefAddr {
        addr: String,
        const_ref: bool,
        observed: bool,
    },
    /// `ref` formal bound to an element of native container storage or to a
    /// member of a native record, through a retained reference that lives
    /// until the call ends.
    ElementRef(Box<IrElementRef>),
    /// Output/inout formal bound to a direct C address (statement calls):
    /// `&G_sig`, a whole-reference address (`o0`, `&_l0`) or a caller-side
    /// temp declared separately (`&_t5`); passed to the callee verbatim.
    OutAddr(String),
    /// Reference formal bound to a canonical typed lvalue. `addr` is diagnostic
    /// provenance only; the ownership emitter constructs its descriptor from
    /// `lhs` and `read` at the call boundary, never by executing a C fragment.
    RefAddr {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        const_ref: bool,
        /// Typed caller-side lvalue used for dependency and write analysis.
        lhs: Box<IrLhs>,
        /// Caller-side read used for dependency/effect analysis. The C
        /// descriptor evaluates the address separately at the call boundary.
        read: Box<IrExpr>,
    },
    /// Output/inout formal bound to a registered caller-side temporary.
    /// `init` is `None` for outputs (a typed default for automatic storage)
    /// and the actual's current value for inouts. `writeback` receives the
    /// formal's value after return, not a caller value on entry.
    OutTemp {
        name: String,
        init: Option<Box<IrExpr>>,
        writeback: Box<IrLhs>,
        /// Optional persistent formal storage used by a static function call.
        /// The caller-side temp stages copy-in only when `init` is present.
        /// An output does not reset this persistent storage. The C call receives
        /// this address and copy-back reads it after return.
        storage_addr: Option<String>,
        storage_lhs: Option<Box<IrLhs>>,
        storage_read: Option<Box<IrExpr>>,
        /// Caller-side selector values captured before the callee runs. Each
        /// tuple is `(name, width, signed, two_state, initializer)`.
        selector_inits: Vec<(String, u32, bool, bool, IrExpr)>,
    },
    /// Caller-side temp for an output/inout native string formal.
    StringOutTemp {
        name: String,
        init: Option<Box<IrStringExpr>>,
        writeback: String,
        storage_addr: Option<String>,
        storage_read: Option<Box<IrStringExpr>>,
    },
}

/// What a [`IrCallArg::ElementRef`] formal addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrElementRefKind {
    String,
    Handle,
    /// A native record: the formal takes the value's address.
    Record,
    /// A packed leaf: the formal takes an `llg_ref_t` naming it.
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
}

/// The value a [`IrElementRef`] starts from.
#[derive(Clone, Debug, PartialEq)]
pub enum IrElementRoot {
    /// One element of descriptor-backed container storage, selected at the
    /// call: `index` for positions and integral keys, `key` for a string key.
    Element {
        container: usize,
        index: Option<IrExpr>,
        key: Option<IrStringExpr>,
    },
    /// A native value root of the caller.
    Value(usize),
    /// One leaf of a module, static or block record, whose leaves have their
    /// own stable storage, selected by a run-time index among their declared
    /// indices. An index selecting none binds a private default (SV 7.4.6).
    Leaves {
        index: IrExpr,
        leaves: Vec<(i32, IrLeafTarget)>,
    },
}

/// The storage of one leaf of [`IrElementRoot::Leaves`].
#[derive(Clone, Debug, PartialEq)]
pub enum IrLeafTarget {
    /// The address of a string or handle object.
    Address(String),
    /// A packed leaf as a typed lvalue with its read, as in
    /// [`IrCallArg::RefAddr`].
    Packed { lhs: IrLhs, read: IrExpr },
}

/// One selection below an [`IrElementRoot`], evaluated at the call.
#[derive(Clone, Debug, PartialEq)]
pub enum IrElementStep {
    /// A record member, or a constant position in a member array.
    Item(u32),
    /// A run-time position in a member array.
    Position(IrExpr),
}

/// The storage a `ref` formal binds through a retained reference.
#[derive(Clone, Debug, PartialEq)]
pub struct IrElementRef {
    pub(in crate::sim) root: IrElementRoot,
    pub(in crate::sim) steps: Vec<IrElementStep>,
    pub(in crate::sim) kind: IrElementRefKind,
    pub(in crate::sim) const_ref: bool,
}

impl IrElementRef {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match &self.root {
            IrElementRoot::Element { index, key, .. } => {
                if let Some(index) = index {
                    visit(index);
                }
                if let Some(key) = key {
                    key.expressions(visit);
                }
            }
            IrElementRoot::Value(_) => {}
            IrElementRoot::Leaves { index, leaves } => {
                visit(index);
                for (_, leaf) in leaves {
                    if let IrLeafTarget::Packed { lhs, read } = leaf {
                        lhs.expressions(visit);
                        visit(read);
                    }
                }
            }
        }
        for step in &self.steps {
            if let IrElementStep::Position(position) = step {
                visit(position);
            }
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match &mut self.root {
            IrElementRoot::Element { index, key, .. } => {
                if let Some(index) = index {
                    visit(index);
                }
                if let Some(key) = key {
                    key.expressions_mut(visit);
                }
            }
            IrElementRoot::Value(_) => {}
            IrElementRoot::Leaves { index, leaves } => {
                visit(index);
                for (_, leaf) in leaves {
                    if let IrLeafTarget::Packed { lhs, read } = leaf {
                        lhs.expressions_mut(visit);
                        visit(read);
                    }
                }
            }
        }
        for step in &mut self.steps {
            if let IrElementStep::Position(position) = step {
                visit(position);
            }
        }
    }
}

/// A function/task call used in expression position. The ownership emitter
/// sequences argument capture, invocation, cancellation and copy-back as C11
/// statements, returning an independently owned result slot.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCallExpr {
    /// Callee index into [`IrModel::funcs`].
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    /// Recursion depth argument at the call site.
    pub(in crate::sim) depth: IrDepth,
    /// Optional hidden receiver passed before ordinary method formals.
    pub(in crate::sim) receiver: Option<IrChandleExpr>,
    /// Dispatch through the callee's virtual slot using the runtime class id.
    pub(in crate::sim) virtual_dispatch: bool,
    /// Optional virtual-interface dispatch metadata. The receiver is kept
    /// separate from class receivers because the generated dispatcher owns
    /// the rebinding lookup rather than a concrete function body.
    pub(in crate::sim) virtual_call: Option<IrVirtualCall>,
    /// Void callee used as a value: yield all-X (warning issued at lowering).
    pub(in crate::sim) void_x: bool,
    /// Index into [`IrModel::dpi_sites`] of this call's source position when
    /// the callee is a context DPI-C import (`svGetCallerInfo`, H.9.3).
    pub(in crate::sim) dpi_site: Option<u32>,
    /// Design node of the source call, which [`IrModel::vpi_call_rows`]
    /// maps to its VPI call object: the `vpiOrigin` of the frame the call
    /// activates (37.39). `None` for a call the source does not write.
    pub(in crate::sim) vpi_call: Option<u32>,
}

impl IrCallExpr {
    /// Create a call-expression staging value. The containing model validates
    /// the function index, arguments, and formal bindings.
    pub fn new(f: usize, args: Vec<IrCallArg>, depth: IrDepth, void_x: bool) -> Self {
        Self {
            f,
            args,
            depth,
            receiver: None,
            virtual_dispatch: false,
            virtual_call: None,
            void_x,
            dpi_site: None,
            vpi_call: None,
        }
    }

    /// Bind a class/object receiver to this call while preserving the normal
    /// typed formal ABI for the remaining arguments.
    pub fn with_receiver(mut self, receiver: IrChandleExpr) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_virtual_call(mut self, virtual_call: IrVirtualCall) -> Self {
        self.virtual_call = Some(virtual_call);
        self
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn yields_x_for_void(&self) -> bool {
        self.void_x
    }
    /// Record the source position of a context DPI-C import call.
    pub fn with_dpi_site(mut self, site: Option<u32>) -> Self {
        self.dpi_site = site;
        self
    }
    pub fn dpi_site(&self) -> Option<u32> {
        self.dpi_site
    }
}

/// A function/task call used in statement position: the temp declarations
/// for select/array-element output actuals (`sv4_t _aN = …`), the C call
/// itself, and the copy-out assignments back into the actuals — one emission
/// unit rendered as a single indented block.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCall {
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    pub(in crate::sim) depth: IrDepth,
    /// Optional hidden receiver passed before ordinary method formals.
    pub(in crate::sim) receiver: Option<IrChandleExpr>,
    /// Dispatch through the callee's virtual slot using the runtime class id.
    pub(in crate::sim) virtual_dispatch: bool,
    /// Optional virtual-interface dispatch metadata.
    pub(in crate::sim) virtual_call: Option<IrVirtualCall>,
    /// `(temp name, formal index, init)` triples declared right before the
    /// call; `init` is `None` for outputs (all-X temp sized by the formal)
    /// and the actual's current value for inouts.
    pub(in crate::sim) temps: Vec<(String, usize, Option<IrExpr>)>,
    /// Copy-outs after the call: `(actual LHS, temp name, width, signed)`.
    pub(in crate::sim) copyouts: Vec<(IrLhs, String, u32, bool)>,
    /// See [`IrCallExpr::dpi_site`].
    pub(in crate::sim) dpi_site: Option<u32>,
    /// See [`IrCallExpr::vpi_call`].
    pub(in crate::sim) vpi_call: Option<u32>,
}

impl IrCall {
    /// Create a statement-call staging value for validation by its model.
    pub fn new(
        f: usize,
        args: Vec<IrCallArg>,
        depth: IrDepth,
        temps: Vec<(String, usize, Option<IrExpr>)>,
        copyouts: Vec<(IrLhs, String, u32, bool)>,
    ) -> Self {
        Self {
            f,
            args,
            depth,
            receiver: None,
            virtual_dispatch: false,
            virtual_call: None,
            temps,
            copyouts,
            dpi_site: None,
            vpi_call: None,
        }
    }

    /// This call carrying the source call `vpi_call` of the expression it
    /// was lowered from ([`IrCallExpr::vpi_call`]).
    pub(in crate::sim) fn with_vpi_call(mut self, vpi_call: Option<u32>) -> Self {
        self.vpi_call = vpi_call;
        self
    }

    /// Bind a class/object receiver to this statement call.
    pub fn with_receiver(mut self, receiver: IrChandleExpr) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_virtual_call(mut self, virtual_call: IrVirtualCall) -> Self {
        self.virtual_call = Some(virtual_call);
        self
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn temps(&self) -> &[(String, usize, Option<IrExpr>)] {
        &self.temps
    }
    pub fn copyouts(&self) -> &[(IrLhs, String, u32, bool)] {
        &self.copyouts
    }
    /// See [`IrCallExpr::with_dpi_site`].
    pub fn with_dpi_site(mut self, site: Option<u32>) -> Self {
        self.dpi_site = site;
        self
    }
    pub fn dpi_site(&self) -> Option<u32> {
        self.dpi_site
    }
}

/// Dynamic dispatch metadata for a call through a virtual-interface handle.
/// `function` on the descriptor is still used for the checked formal ABI;
/// emission selects the concrete implementation from the receiver environment.
#[derive(Clone, Debug, PartialEq)]
pub struct IrVirtualCall {
    pub(in crate::sim) interface: usize,
    pub(in crate::sim) method: usize,
    pub(in crate::sim) receiver: IrChandleExpr,
    /// Source site reported when the receiver is null (SV 25.9): the call is
    /// a fatal run-time error and the process does not continue.
    pub(in crate::sim) site: String,
}

/// Recursion depth argument of a call site: `"0"` in process contexts,
/// `"depth + 1"` inside C function bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrDepth {
    /// `true` when the call sits in a C function body.
    pub(in crate::sim) func_base: bool,
}

impl IrDepth {
    pub const PROC: IrDepth = IrDepth { func_base: false };
    pub const FUNC: IrDepth = IrDepth { func_base: true };

    /// The C expression spelling this depth.
    pub fn code(&self) -> String {
        if self.func_base {
            "depth + 1".to_string()
        } else {
            "0".to_string()
        }
    }

    pub fn is_function_based(&self) -> bool {
        self.func_base
    }
}

impl IrCallArg {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::FixedValue(value) => value.expressions(visit),
            Self::Val(value) => visit(value),
            Self::StringVal(value) => value.expressions(visit),
            Self::ChandleVal(value) => value.expressions(visit),
            Self::RefAddr { lhs, read, .. } => {
                lhs.expressions(visit);
                visit(read);
            }
            Self::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } => {
                if let Some(value) = init {
                    visit(value);
                }
                writeback.expressions(visit);
                if let Some(lhs) = storage_lhs {
                    lhs.expressions(visit);
                }
                if let Some(value) = storage_read {
                    visit(value);
                }
                for (_, _, _, _, value) in selector_inits {
                    visit(value);
                }
            }
            Self::StringOutTemp {
                init, storage_read, ..
            } => {
                if let Some(value) = init {
                    value.expressions(visit);
                }
                if let Some(value) = storage_read {
                    value.expressions(visit);
                }
            }
            Self::NativeLeaves { leaves, .. } => {
                for leaf in leaves {
                    leaf.expressions(visit);
                }
            }
            Self::NativeCall { call, .. } => {
                for argument in &call.args {
                    argument.expressions(visit);
                }
            }
            Self::RealArrayValues(values) | Self::ContainerValues { values, .. } => {
                values.iter().for_each(visit)
            }
            Self::RealArrayCall { call, .. } => {
                for argument in &call.args {
                    argument.expressions(visit);
                }
            }
            // An event-array index or handle expression is evaluated at the
            // call site.
            Self::EventVal(event) | Self::EventAddr(event) => event.expressions(visit),
            Self::ElementRef(reference) => reference.expressions(visit),
            Self::FixedArray(_)
            | Self::NativeValue(_)
            | Self::NativeRefBound
            | Self::Container(_)
            | Self::RealArray(_)
            | Self::ChandleAddr(_)
            | Self::ChandleRefAddr(_)
            | Self::EventSlot(_)
            | Self::StringOutAddr(_)
            | Self::StringRefAddr { .. }
            | Self::OutAddr(_) => {}
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::FixedValue(value) => value.expressions_mut(visit),
            Self::Val(value) => visit(value),
            Self::ElementRef(reference) => reference.expressions_mut(visit),
            Self::StringVal(value) => value.expressions_mut(visit),
            Self::ChandleVal(value) => value.expressions_mut(visit),
            Self::RefAddr { lhs, read, .. } => {
                lhs.expressions_mut(visit);
                visit(read);
            }
            Self::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } => {
                if let Some(value) = init {
                    visit(value);
                }
                writeback.expressions_mut(visit);
                if let Some(lhs) = storage_lhs {
                    lhs.expressions_mut(visit);
                }
                if let Some(value) = storage_read {
                    visit(value);
                }
                for (_, _, _, _, value) in selector_inits {
                    visit(value);
                }
            }
            Self::StringOutTemp {
                init, storage_read, ..
            } => {
                if let Some(value) = init {
                    value.expressions_mut(visit);
                }
                if let Some(value) = storage_read {
                    value.expressions_mut(visit);
                }
            }
            Self::NativeLeaves { leaves, .. } => {
                for leaf in leaves {
                    leaf.expressions_mut(visit);
                }
            }
            Self::NativeCall { call, .. } => {
                for argument in &mut call.args {
                    argument.expressions_mut(visit);
                }
            }
            Self::RealArrayValues(values) | Self::ContainerValues { values, .. } => {
                values.iter_mut().for_each(visit)
            }
            Self::RealArrayCall { call, .. } => {
                for argument in &mut call.args {
                    argument.expressions_mut(visit);
                }
            }
            Self::EventVal(event) | Self::EventAddr(event) => event.expressions_mut(visit),
            Self::FixedArray(_)
            | Self::NativeValue(_)
            | Self::NativeRefBound
            | Self::Container(_)
            | Self::RealArray(_)
            | Self::ChandleAddr(_)
            | Self::ChandleRefAddr(_)
            | Self::EventSlot(_)
            | Self::StringOutAddr(_)
            | Self::StringRefAddr { .. }
            | Self::OutAddr(_) => {}
        }
    }
}

/// Reserved local binding for a captured input in the current call frame.
pub(in crate::sim) fn call_argument_name(index: usize) -> String {
    format!("__llg_call_argument_{index}")
}
