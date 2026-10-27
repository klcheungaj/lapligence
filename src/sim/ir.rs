//! ir — typed intermediate representation for the simulator model.
//!
//! The simulator pipeline is `core::db` → [`crate::sim::semantic::SemanticModel`]
//! → typed operation staging here → [`crate::sim::execution::ExecutionModel`]
//! → optimization passes ([`crate::sim::opt`]) → C11 text
//! ([`crate::sim::emit_c`]). Every lowering decision — widths, signedness,
//! unsized-fill markers, sensitivity/read sets, timescale scaling, C names —
//! is made once at lowering time and recorded here; the backend renders the
//! recorded decisions verbatim and the optimizer transforms the model
//! conservatively without recomputing any wake behavior.
//!
//! IR conventions:
//!
//! - an expression whose `width` is 0 is a *real* (double) value; packed
//!   values are never zero-width (`REAL_EXPR_WIDTH`);
//! - `fill: Option<u8>` marks a bare unsized fill literal (`'0`/`'1`/`'x`/`'z`,
//!   bit values 0/1/2=x/3=z); assignment/call/return conversions render it as
//!   `sv4_fill` instead of `sv4_resize`;
//! - signal/array/function references are indices into the [`IrModel`] tables,
//!   which are complete before any statement lowering runs;
//! - process order equals spawn order (comb, links, then always/initial;
//!   final blocks are rendered with the rest but spawn into a separate
//!   post-simulation phase).

mod containers;
pub(in crate::sim) use containers::shared_container_capture_name;
pub(in crate::sim) use native_values::shared_native_capture_name;
mod checkers;
pub use checkers::IrFreeSet;
mod native_access;
pub use native_access::{IrClassAllocation, IrNativeAccess, IrNativeAccessKind};
mod native_values;
pub(in crate::sim) use native_values::{
    is_class_handle_kind, native_item_count, native_leaf_type, validate_native_type,
};
pub use native_values::{IrNativeLeafExpr, IrNativeLeafValue, IrNativeValue};
mod objects;
mod patterns;
pub use patterns::{
    IrPattern, IrPatternClass, IrPatternEnumMember, IrPatternField, IrPatternMember,
    IrPatternSource, IrPatternStorage, IrPatternType,
};
mod validate;
pub use containers::{
    IrAssocKey, IrAssocTraversal, IrBitStream, IrContainer, IrContainerElement, IrContainerExpr,
    IrContainerKind, IrContainerMember, IrContainerMethod, IrContainerReduction, IrContainerStmt,
    IrQueueBound, IrQueueSource, IrStreamSegment, IrStreamSelector, IrValueItemRoot, IrValueSlot,
    PROCESS_ELEMENT_KIND,
};
pub use objects::{
    IrArrayDimension, IrArrayQuery, IrArrayQueryKind, IrArrayQueryTarget, IrChandleExpr, IrClass,
    IrClassField, IrClassFieldType, IrDisplayArg, IrMailboxElement, IrMailboxExpr, IrMailboxTarget,
    IrMailboxValue, IrObject, IrObjectQuery, IrObjectStmt, IrObjectType, IrProcessControl,
    IrProcessExpr, IrProcessRandom, IrStringExpr, IrStringInsideItem, IrVirtualInterface,
    IrVirtualInterfaceInstance, IrVirtualInterfaceMember, IrVirtualInterfaceMethod,
};

pub use validate::IrValidationError;

mod constants;
pub use constants::IrConst;
mod fixed_values;
pub use fixed_values::IrFixedValue;

mod fixed_array_cells;
pub(in crate::sim) use fixed_array_cells::FixedArrayCellShape;
pub use fixed_array_cells::{
    IrFixedArrayCells, IrFixedArrayOrder, IrFixedArrayOrderMethod, IrRealArrayOrder,
};
mod fixed_array_reductions;
pub use fixed_array_reductions::{IrFixedArrayReduction, IrFixedArrayReductionSource};
mod timing;
pub use timing::{
    expand_path_delays, IrModulePath, IrModulePathDestination, IrModulePathSet, IrPathCondition,
    IrPathEdge, IrPathLimits, IrPathPolarity, IrPathPulseOptions, IrPulseStyle, IrTimingCheck,
    IrTimingCheckKind, IrTimingCheckOptions, IrTimingCheckSet, IrTimingCondition, IrTimingEvent,
    IrTimingLine, IrTimingLineStorage, IrTimingPair, IrTimingTerminal, PATH_STYLE_ON_DETECT,
    PATH_STYLE_SHOW_CANCELLED, PATH_TRANSITIONS, TIMING_DISABLED, TIMING_EDGE_MASK,
    TIMING_EVENT_BASED, TIMING_REMAIN_ACTIVE,
};
mod switches;
pub use switches::{IrSwitch, IrSwitchControl, IrSwitchNetwork, IrSwitchNode};
mod sdf;
pub use sdf::{
    IrSdfAnnotation, IrSdfLoad, IrSdfOp, IrSdfTarget, SDF_INCREMENT, SDF_MIPD_SHIFT, SDF_PERCENT,
};
mod udp;
pub use udp::{
    IrUdpEdge, IrUdpInput, IrUdpNext, IrUdpOutput, IrUdpRow, IrUdpSequential, IrUdpSequentialRow,
    IrUdpTable,
};
mod expressions;
pub use expressions::{
    IrBinOp, IrBitQuery, IrDynamicCast, IrEnumMember, IrEnumMethod, IrEnumQuery, IrExpr,
    IrExprKind, IrFileInput, IrFileInputTarget, IrFileReadTarget, IrLeafMerge, IrMathFunc,
    IrMutationExpr, IrNativeBinding, IrPatternCheck, IrPatternExpr, IrPatternMatchKind,
    IrPlusArgTarget, IrPlusArgText, IrRandomFunc, IrRealBinOp, IrRealUnOp, IrRuntimeQuery,
    IrSampledCall, IrSampledClock, IrSampledClockKind, IrSampledDomain, IrSampledFunc, IrScanScope,
    IrSequenceExpr, IrSysFunc, IrTaggedMemberGuard, IrTaggedSelectStep, IrTimeKind, IrUnOp,
};
mod lvalues;
pub use lvalues::{
    IrElemSel, IrInsideArrayElement, IrInsideItem, IrLhs, IrPackedSelect, IrStreamDirection,
    IrStreamTarget,
};
mod calls;
pub(in crate::sim) use calls::call_argument_name;
pub use calls::{IrCall, IrCallArg, IrCallExpr, IrDepth, IrVirtualCall};
mod coverage;
pub use coverage::{
    IrAssertionCoverage, IrAssertionItem, IrAssertionItemForm, IrAssertionItemKind,
    IrCoverageApiCall, IrCoverageApiFunction, IrCoverageInstance, IrCoverageScope,
};
pub use coverage::{
    IrCoverAutomatic, IrCoverBin, IrCoverBinKind, IrCoverBinShape, IrCoverBound, IrCoverCount,
    IrCoverCross, IrCoverCrossBin, IrCoverItem, IrCoverRepeat, IrCoverSelect, IrCoverTransBin,
    IrCoverTransSet, IrCoverTransStep, IrCoverTransitions, IrCoverageCrossSample, IrCoverageOption,
    IrCoverageQuery, IrCoverageSample, IrCoverageStmt, IrCoverageText, IrCoverageTextQuery,
    IrCoverageType, IrCoverageTypeItem, IrCoverageTypeOption, IrCovergroup, IrCoverpoint,
};
mod randomize;
pub use randomize::{
    rand_cyclic_state_width, IrRandArray, IrRandArrayKind, IrRandBlock, IrRandCall, IrRandDist,
    IrRandDistItem, IrRandLeaf, IrRandNode, IrRandOp, IrRandOrder, IrRandProblem, IrRandQuery,
    IrRandSlot, IrRandStmt, IrRandUnit, IrRandVar, RAND_CYCLIC_MAX_WIDTH, RAND_NONE,
    RAND_SLOT_ARRAY, RAND_STEP_FAILED, RAND_STEP_NEED_CALLS, RAND_STEP_OK,
};
mod statements;
pub use statements::{
    IrActivationTarget, IrCaseItem, IrCaseKind, IrClockingSampleMode, IrDisplayRadix, IrFileOp,
    IrMemoryAccess, IrMemoryAddressingPolicy, IrMemoryImage, IrMemoryRadix, IrMemorySelector,
    IrMemorySelectorKind, IrMemoryStorage, IrMemoryView, IrStmt, IrStochasticStmt,
    IrUniquePriorityCheck, IrWaveDumpVars,
};
mod events;
pub use events::{
    IrCapture, IrCapturedBranch, IrDeferredAction, IrDelay, IrEdge, IrEvent, IrEventCapture,
    IrEventContext, IrEventRef, IrInertialDelay, IrJoinKind, IrRuntimeTransitionDelay,
    IrTransitionDelay, IrWaitSrc,
};
mod assertions;
pub use assertions::{
    IrAssertion, IrAssertionControlKind, IrAssertionEndpoint, IrAssertionFuture,
    IrConcurrentAssertionKind, IrImmediateAssertionKind, IrProperty, IrPropertyBinaryOp,
    IrPropertyClock, IrPropertyNode, IrSequence, IrSequenceJoin, IrSequenceJoinKind,
    IrSequenceLocal, IrSequenceRange, IrSequenceTransition, IrSeverityLevel,
};
mod processes;
pub use processes::{IrPreFn, IrProcess, IrProcessKind, IrShape};
mod functions;
pub(in crate::sim) use functions::dimension_size;
pub use functions::{IrDpiExport, IrDpiImport, IrDpiType, IrFormal, IrFormalMode, IrFunc, IrLocal};
mod initialization;
pub use initialization::{IrInitPhase, IrInitStep, IrInitTarget, IrInitialization};
mod storage;
pub use storage::{
    IrArray, IrNetAliasBinding, IrNetArray, IrNetCellRun, IrNetGroup, IrNetKind, IrSignal,
};
mod vpi;
pub use vpi::{
    IrVpiArgRef, IrVpiBuiltinCall, IrVpiCompileArg, IrVpiCompileCall, IrVpiConn, IrVpiConnRole,
    IrVpiConstant, IrVpiDirection, IrVpiObject, IrVpiObjectKind, IrVpiTiming, IrVpiTypespec,
    IrVpiTypespecKind, IrVpiTypespecMember, IrVpiVarKind,
};
mod vpi_dynamic;
pub use vpi_dynamic::{
    IrVpiArrayType, IrVpiClass, IrVpiDynamic, IrVpiField, IrVpiKey, IrVpiShape, IrVpiStorage,
};

/// Upper bound on one generated `llg_net_t`'s driver count. The runtime stores
/// the count in a C `int`; this is a representation limit checked before
/// emission, not an admission ceiling on legal net connectivity.
pub const LLG_MAX_NET_DRIVERS: usize = i32::MAX as usize;
/// Maximum number of arguments exposed through one generated VPI call.
pub const LLG_MAX_VPI_ARGS: usize = 256;
/// Maximum number of cells in one generated fixed unpacked array.
///
/// Large integral variable arrays use lazy stable cells and an owned default;
/// direct reductions still traverse logical cells. This is a generated-model resource
/// ceiling, not a language or packed-value width limit. Lowering and IR
/// validation check the dimension product before cell tables or C declarations
/// are created.
pub const LLG_MAX_FIXED_ARRAY_CELLS: u64 = 1 << 24;

/// Integral variable arrays above this count use lazy, stable cell storage.
pub const LLG_DENSE_FIXED_ARRAY_CELLS: u64 = 4096;

/// Fewest consecutive fixed-array cells that a whole-value read or write accesses
/// with one runtime range call instead of one access per element (emitter
/// `owned/dense_cells.rs`); lowering captures whole-array sources of at least
/// this many cells as one image so their element stores form such a run.
/// See `docs/sim_performance_tradeoffs.md` for the measurement behind it.
pub const LLG_DENSE_CELL_RUN_MIN: usize = 6;

fn validate_width(path: &str, width: u32) -> Result<(), IrValidationError> {
    if width != 0 {
        Ok(())
    } else {
        Err(IrValidationError::new(path, "packed width must be nonzero"))
    }
}

/// A lowered storage type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IrType {
    /// A packed 4-state vector (`sv4_t`).
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    /// A real scalar stored in a companion `double` global.
    Real {
        /// `true` for `shortreal` (values round through C `float`).
        shortreal: bool,
    },
}

/// Stable identity for one activation frame.
///
/// A frame id is assigned while lowering a fork capture site.  It is kept
/// separate from generated C names so later passes can reason about storage
/// ownership without treating an emitter spelling as an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(u32);

impl FrameId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Lifetime class of a typed storage descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageLifetime {
    /// One model-wide/static declaration.
    Static,
    /// One invocation or lexical block activation.
    Automatic,
}

/// Ownership mode of a typed storage descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageOwnership {
    /// The owner is an enclosing activation and must outlive this use.
    Borrowed,
    /// This frame owns a cloned value until the capture completes.
    Owned,
    /// Multiple child activations retain one shared frame.
    Shared,
}

/// Value representation carried by an activation slot.
///
/// This is deliberately independent of [`IrType`]: a storage descriptor
/// identifies ownership and lifetime, while the expression type identifies
/// how a value is evaluated.  Keeping the two separate lets retained frames
/// grow to strings/aggregates without making a C pointer the storage ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageKind {
    /// Four-state packed storage (`sv4_t`).
    Packed,
    /// IEEE real/shortreal storage (`double` in the runtime frame).
    Real,
    /// A runtime-owned object handle copied into an activation frame.  The
    /// handle itself remains owned by the runtime object registry; frames do
    /// not retain host pointers or attempt to clone/drop the object.
    Opaque,
    /// A named-event handle: a borrowed parent handle or copied object identity.
    Event,
    /// An owned string value (`llg_string_t`), or a shared frame's string.
    String,
    /// An activation container in a shared frame's object slot.
    Container,
    /// An activation native value (`llg_native_root_t`) in slot 0 of a shared
    /// frame whose later slots hold its companion containers.
    Native,
}

/// A typed reference to one slot in an activation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageRef {
    frame: FrameId,
    slot: u32,
    declaration: u32,
    lifetime: StorageLifetime,
    ownership: StorageOwnership,
    kind: StorageKind,
}

impl StorageRef {
    pub const fn new(
        frame: FrameId,
        slot: u32,
        lifetime: StorageLifetime,
        ownership: StorageOwnership,
    ) -> Self {
        Self {
            frame,
            slot,
            declaration: u32::MAX,
            lifetime,
            ownership,
            kind: StorageKind::Packed,
        }
    }

    pub const fn for_declaration(
        frame: FrameId,
        slot: u32,
        declaration: u32,
        lifetime: StorageLifetime,
        ownership: StorageOwnership,
    ) -> Self {
        Self {
            frame,
            slot,
            declaration,
            lifetime,
            ownership,
            kind: StorageKind::Packed,
        }
    }

    pub const fn frame(self) -> FrameId {
        self.frame
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn declaration(self) -> Option<u32> {
        if self.declaration == u32::MAX {
            None
        } else {
            Some(self.declaration)
        }
    }

    pub const fn lifetime(self) -> StorageLifetime {
        self.lifetime
    }

    pub const fn ownership(self) -> StorageOwnership {
        self.ownership
    }

    pub const fn kind(self) -> StorageKind {
        self.kind
    }

    pub const fn with_kind(mut self, kind: StorageKind) -> Self {
        self.kind = kind;
        self
    }
}

/// A stable storage dependency used by sensitivity-driven processes and waits.
///
/// Dependency keys identify storage rather than transient addresses.  In
/// particular, resizable containers use their contents/shape keys instead of
/// retaining pointers into allocations that a resize or delete may replace.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IrDependency {
    /// Static bit interval within a packed scalar or fixed-array element.
    PackedRange {
        storage: Box<IrDependency>,
        lsb: u32,
        width: u32,
    },
    /// A scalar packed value (the string is the generated C storage name).
    Scalar(String),
    /// A scalar real/shortreal value (the string is the generated C storage
    /// name). Real storage is kept distinct from packed `sv4_t` storage so
    /// wait and sensitivity lowering cannot accidentally use vector helpers.
    Real(String),
    /// One element of a fixed unpacked array, in flattened storage order.
    ArrayElement { array: usize, index: u64 },
    /// Any value in a fixed unpacked array.
    ArrayContents(usize),
    /// A value in a dynamic/queue/associative container.
    ContainerContents(usize),
    /// Container membership/size/shape (including insertion/deletion).
    ContainerShape(usize),
    /// A persistent native string object. The generated model gives each
    /// string object a stable packed change marker used by link processes.
    Object(usize),
    /// The fork-shared activation cell bound to the waiting process's local
    /// `local` (packed, or real when `real`). Sharing processes write it
    /// through publishing stores, so its address is a wait source.
    SharedCell { local: String, real: bool },
    /// The change marker of the fork-shared string (or, when `handle`, the
    /// handle) the waiting process names `local`: a string carries the
    /// marker its frame owns; a handle's frame slot keeps it beside the
    /// handle (`llg_frame_opaque_marker`).
    SharedNative { local: String, handle: bool },
    /// The storage behind `ref` formal `index` of the enclosing typed
    /// subroutine, named at run time through its descriptor (a real `ref`
    /// formal is the cell address itself): the variables, element cells or
    /// class properties it names, the leaves of a view's or composite's
    /// storage, or the contents marker of the container whose element it
    /// retains. A packed descriptor may name several storages (an unpacked
    /// aggregate formal, or a view into one), so only dependency waits,
    /// which expand it when they arm, name it as a dependency; plain and
    /// edge controls use [`IrWaitSrc::RefFormal`].
    RefFormal { index: usize, real: bool },
    /// The change marker of the string (or, when `handle`, the class
    /// handle variable) that string or handle `ref` formal `index` of the
    /// enclosing typed subroutine names at run time. Storage without a
    /// marker selects one that never changes.
    NativeRefFormal { index: usize, handle: bool },
    /// The packed or real storage that native access `name` (a class
    /// property or virtual-interface member, see
    /// [`crate::sim::ir::IrNativeAccess`]) currently selects; for a class
    /// handle property, the marker its object toggles when a handle property
    /// is rebound. The receiver is evaluated when the wait arms, so only
    /// waits that arm again after every wake (`WaitAny`, `WaitCond` and
    /// sensitivity loops) may name it; rebinding the receiver then moves the
    /// next wait to the new storage. A null or invalid receiver selects
    /// storage that never changes.
    NativeAccess(String),
    /// The storage of the interface instance that virtual-interface variable
    /// `handle` (an object index) binds when the wait arms: `arms[i]` names
    /// instance `i` of descriptor `interface` (an element or contents marker
    /// of that instance's unpacked array member, `v.mem[i]`). Like
    /// [`IrDependency::NativeAccess`], only waits that arm again after every
    /// wake may name it; a null or unbound handle selects storage that never
    /// changes.
    InterfaceInstance {
        handle: usize,
        interface: usize,
        arms: Vec<IrDependency>,
    },
}

impl IrDependency {
    pub fn scalar(name: impl Into<String>) -> Self {
        Self::Scalar(name.into())
    }

    pub fn real(name: impl Into<String>) -> Self {
        Self::Real(name.into())
    }

    pub fn object(index: usize) -> Self {
        Self::Object(index)
    }

    pub fn scalar_name(&self) -> Option<&str> {
        match self {
            Self::Scalar(name) => Some(name),
            Self::PackedRange { storage, .. } => storage.scalar_name(),
            _ => None,
        }
    }

    pub fn real_name(&self) -> Option<&str> {
        match self {
            Self::Real(name) => Some(name),
            _ => None,
        }
    }
}

impl From<String> for IrDependency {
    fn from(value: String) -> Self {
        Self::Scalar(value)
    }
}

impl From<&str> for IrDependency {
    fn from(value: &str) -> Self {
        Self::Scalar(value.to_owned())
    }
}

impl IrType {
    /// Construct a packed type with a nonzero width, independent of backends.
    pub fn packed(width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("type.width", width)?;
        Ok(Self::Packed {
            width,
            signed,
            two_state: false,
        })
    }

    /// Packed width, or 0 for real types.
    pub fn width(&self) -> u32 {
        match self {
            IrType::Packed { width, .. } => *width,
            IrType::Real { .. } => 0,
        }
    }

    pub fn signed(&self) -> bool {
        match self {
            IrType::Packed { signed, .. } => *signed,
            IrType::Real { .. } => false,
        }
    }

    /// Whether packed storage coerces X/Z to zero.
    pub fn two_state(&self) -> bool {
        matches!(
            self,
            Self::Packed {
                two_state: true,
                ..
            }
        )
    }
}

/// Waveform dumping the driver requested without `$dumpfile`/`$dumpvars`.
///
/// Startup opens `file` and selects `selection` at time zero; the design's own
/// `$dumpfile` and `$dumpvars` calls are then ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrWaveStart {
    pub(in crate::sim) file: String,
    pub(in crate::sim) selection: IrWaveDumpVars,
}

impl IrWaveStart {
    pub fn new(file: String, selection: IrWaveDumpVars) -> Self {
        Self { file, selection }
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn selection(&self) -> &IrWaveDumpVars {
        &self.selection
    }
}

/// Typed operation staging used while building an executable model.
///
/// [`crate::sim::execution::ExecutionModel::lower`] moves process bodies into
/// executable blocks. Optimization and whole-model emission do not accept this
/// staging representation directly.
#[derive(Clone, Debug)]
pub struct IrModel {
    pub(in crate::sim) design_name: String,
    /// Design time precision in fs (scheduler tick unit).
    pub(in crate::sim) precision_fs: u64,
    /// At least one waveform-control system task was lowered, or the driver
    /// requested dumping.
    pub(in crate::sim) waveform: bool,
    /// Driver-requested dumping that starts at time zero.
    pub(in crate::sim) wave_start: Option<IrWaveStart>,
    pub(in crate::sim) signals: Vec<IrSignal>,
    pub(in crate::sim) net_groups: Vec<IrNetGroup>,
    pub(in crate::sim) arrays: Vec<IrArray>,
    pub(in crate::sim) containers: Vec<IrContainer>,
    pub(in crate::sim) objects: Vec<IrObject>,
    /// Nominal class layouts used by class handles and method receivers.
    pub(in crate::sim) classes: Vec<IrClass>,
    /// Typed dynamic member lvalues, resolved at each use rather than C fragments.
    pub(in crate::sim) native_accesses: Vec<IrNativeAccess>,
    /// Recursive types of descriptor-backed native values, emitted once each.
    pub(in crate::sim) native_types: Vec<IrContainerElement>,
    /// Descriptor-backed native aggregate storage (formals, results, locals).
    pub(in crate::sim) native_values: Vec<IrNativeValue>,
    /// Display types of `%p` arguments, children before parents.
    pub(in crate::sim) pattern_types: Vec<IrPatternType>,
    /// Display layout of every class, indexed like `classes`; empty when no
    /// `%p` argument can reach a class object.
    pub(in crate::sim) pattern_classes: Vec<IrPatternClass>,
    pub(in crate::sim) class_allocations: Vec<IrClassAllocation>,
    /// Virtual-interface descriptors and their concrete instance bindings.
    pub(in crate::sim) virtual_interfaces: Vec<IrVirtualInterface>,
    /// User-defined primitive definitions, shared across gate instances.
    pub(in crate::sim) udp_tables: Vec<IrUdpTable>,
    /// Sequential UDP instance states: the `udp_tables` index of each
    /// instance's definition. Each entry is one persistent state record that
    /// only that instance's [`IrStmt::UdpStep`] updates.
    pub(in crate::sim) udp_states: Vec<usize>,
    /// Simple module paths, one set per module instance that declares any
    /// (IEEE 1800-2009 30.4). Empty for designs without specify paths.
    pub(in crate::sim) module_paths: Vec<IrModulePathSet>,
    /// Bidirectional pass switches (IEEE 1800-2009 28.8); empty for designs
    /// without them.
    pub(in crate::sim) switch_network: IrSwitchNetwork,
    /// Global pulse controls of every module path (30.7.2, 30.7.4).
    pub(in crate::sim) path_pulse: IrPathPulseOptions,
    /// System timing checks, one set per module instance that declares any
    /// (IEEE 1800-2009 31). Empty for designs without timing checks.
    pub(in crate::sim) timing_checks: Vec<IrTimingCheckSet>,
    /// Global timing check controls.
    pub(in crate::sim) timing_check_options: IrTimingCheckOptions,
    /// The hidden one-bit two-state signal the runtime toggles to wake the
    /// timing process; present exactly when `timing_checks` is not empty.
    pub(in crate::sim) timing_doorbell: Option<usize>,
    /// SDF annotations (IEEE 1800-2009 32), applied by
    /// [`IrStmt::SdfAnnotate`] or, listed in `sdf_startup`, before
    /// simulation starts. Empty without `$sdf_annotate` and `--sdf`.
    pub(in crate::sim) sdf_annotations: Vec<IrSdfAnnotation>,
    pub(in crate::sim) sdf_startup: Vec<u32>,
    /// The writable rise, fall and turn-off delays of each gate an SDF
    /// annotation targets, initialized with the gate's own delays.
    pub(in crate::sim) sdf_gate_delays: Vec<IrTransitionDelay>,
    /// The load port bits SDF interconnect delays annotate.
    pub(in crate::sim) sdf_loads: Vec<IrSdfLoad>,
    pub(in crate::sim) events: Vec<IrEvent>,
    pub(in crate::sim) funcs: Vec<IrFunc>,
    /// Concurrent assertion instances, kept outside ordinary process IR.
    pub(in crate::sim) assertions: Vec<IrAssertion>,
    /// Sampled-value clocks and the expression histories sampled on them.
    pub(in crate::sim) sampled_clocks: Vec<IrSampledClock>,
    pub(in crate::sim) sampled_domains: Vec<IrSampledDomain>,
    /// Comb drivers, then links, then always/initial processes — push order
    /// equals spawn order.
    pub(in crate::sim) processes: Vec<IrProcess>,
    pub(in crate::sim) init_steps: Vec<IrInitStep>,
    /// Spawned function names in spawn order (labels resolve through
    /// `processes`).  Final-block processes are NOT in this list; they run
    /// after the scheduler exits (see `final_spawns`).
    pub(in crate::sim) spawns: Vec<String>,
    /// Final-block process function names (`final begin … end`, SV
    /// 1800-2005 §10.7) in spawn order.  The backend registers them via
    /// `llg_spawn_final` and runs them with `llg_rt_run_finals()` AFTER
    /// `llg_rt_run()` returns ($finish / deadlock / no future events).
    pub(in crate::sim) final_spawns: Vec<String>,
    /// Stable owned VPI object identities captured before lowering storage is
    /// optimized. Empty for hand-built IR fixtures that do not enable VPI.
    pub(in crate::sim) vpi_objects: Vec<IrVpiObject>,
    /// Interned typespecs the catalog objects name (IEEE 1800-2009 37.23).
    pub(in crate::sim) vpi_typespecs: Vec<IrVpiTypespec>,
    /// Value shapes and class layouts of the dynamic catalog objects
    /// (strings, handles, containers, class objects; 37.17, 37.29).
    pub(in crate::sim) vpi_dynamic: IrVpiDynamic,
    /// Type-only VPI call sites run through compiletf/sizetf before the
    /// generated scheduler starts. Empty for hand-built IR fixtures.
    pub(in crate::sim) vpi_compile_calls: Vec<IrVpiCompileCall>,
    /// Built-in system subroutines run natively, by name, with their first
    /// call site; checked against VPI registrations at startup. Empty for
    /// hand-built IR fixtures.
    pub(in crate::sim) vpi_builtin_calls: Vec<IrVpiBuiltinCall>,
    /// Whether the model emits its VPI object catalog. Generation clears it,
    /// with `vpi_objects`, when no application can reach the catalog: no
    /// user system task call site and no `CodegenOptions::vpi`.
    pub(in crate::sim) vpi_catalog: bool,
    /// Whether every delay the catalog names is writable runtime storage
    /// (models generated for VPI applications), which `vpi_put_delays`
    /// needs.
    pub(in crate::sim) vpi_delays_writable: bool,
    /// The min:typ:max operand the model's delays use (0 min, 1 typ, 2
    /// max), which `vpi_put_delays` takes from min:typ:max values.
    pub(in crate::sim) vpi_delay_select: u8,
    /// Random streams the model observes; set by lowering.
    pub(in crate::sim) random: IrRandomUse,
    /// Constraint problems of randomize calls (IEEE 1800-2009 18), one per
    /// call site and dynamic class; each owns one runtime solver session.
    pub(in crate::sim) rand_problems: Vec<IrRandProblem>,
    /// Active free checker variables, ascending: assertions and other
    /// sampled contexts read their current value (IEEE 1800-2009 17.7.2).
    pub(in crate::sim) free_variables: Vec<usize>,
    /// Assume-based randomization of active free checker variables, one set
    /// per checker instance that has them.
    pub(in crate::sim) free_sets: Vec<IrFreeSet>,
    /// Elaborated covergroup declarations (IEEE 1800-2009 19), indexed by
    /// coverage statements and queries.
    pub(in crate::sim) covergroups: Vec<IrCovergroup>,
    /// Covergroup declaration identities whose instances type coverage merges.
    pub(in crate::sim) coverage_types: Vec<IrCoverageType>,
    /// Assertion coverage items and the instance hierarchy of `$coverage_*`
    /// scopes (40.3.2). Present only when the design calls a coverage
    /// access function or the model emits its VPI catalog, so other models
    /// pay nothing for assertion coverage. Set by lowering.
    pub(in crate::sim) assertion_coverage: Option<IrAssertionCoverage>,
    /// Instantiated scopes holding DPI-C import or export declarations, in
    /// ascending byte order of their names; an `svScope` names one (SV
    /// Annex H.9). Set by lowering.
    pub(in crate::sim) dpi_scopes: Vec<IrDpiScope>,
    /// Source positions of context import calls for `svGetCallerInfo`
    /// (H.9.3). Set by lowering.
    pub(in crate::sim) dpi_sites: Vec<IrDpiSite>,
}

/// One instantiated scope of DPI-C declarations (SV Annex H.9.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrDpiScope {
    /// Fully qualified scope name, as `svGetNameFromScope` returns it.
    pub(in crate::sim) name: String,
}

impl IrDpiScope {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The source file and line of one context import call (H.9.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrDpiSite {
    pub(in crate::sim) file: String,
    pub(in crate::sim) line: u32,
}

impl IrDpiSite {
    pub fn new(file: String, line: u32) -> Self {
        Self { file, line }
    }
}

/// Which random streams a model observes (SV 18.14). Each flag enables a
/// per-object cost only in models that can observe its effect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IrRandomUse {
    /// Thread streams are drawn or inspected, so object creation consumes
    /// the creating thread's next value (18.14.1 object stability).
    pub threads: bool,
    /// Object streams are seeded or inspected, so every class object keeps
    /// its own stream state.
    pub objects: bool,
}

/// Staging tables for constructing an [`IrModel`].
///
/// These fields deliberately carry no invariant by themselves. Pass the
/// completed value to [`IrModel::from_parts`], which validates every table
/// index, storage shape, process registration, and nested IR node before it
/// returns an invariant-bearing model.
#[derive(Clone, Debug, Default)]
pub struct IrModelParts {
    pub waveform: bool,
    pub wave_start: Option<IrWaveStart>,
    pub signals: Vec<IrSignal>,
    pub net_groups: Vec<IrNetGroup>,
    pub arrays: Vec<IrArray>,
    pub containers: Vec<IrContainer>,
    pub objects: Vec<IrObject>,
    pub classes: Vec<IrClass>,
    pub native_accesses: Vec<IrNativeAccess>,
    pub class_allocations: Vec<IrClassAllocation>,
    pub virtual_interfaces: Vec<IrVirtualInterface>,
    pub udp_tables: Vec<IrUdpTable>,
    pub events: Vec<IrEvent>,
    pub funcs: Vec<IrFunc>,
    pub assertions: Vec<IrAssertion>,
    pub sampled_clocks: Vec<IrSampledClock>,
    pub sampled_domains: Vec<IrSampledDomain>,
    pub processes: Vec<IrProcess>,
    pub init_steps: Vec<IrInitStep>,
    pub spawns: Vec<String>,
    pub final_spawns: Vec<String>,
}

impl IrModel {
    /// Start an incrementally lowered model with a valid scheduler precision.
    pub fn new(design_name: String, precision_fs: u64) -> Result<Self, IrValidationError> {
        Self::from_parts(design_name, precision_fs, IrModelParts::default())
    }

    /// Build a complete model and validate all representation invariants.
    pub fn from_parts(
        design_name: String,
        precision_fs: u64,
        parts: IrModelParts,
    ) -> Result<Self, IrValidationError> {
        if precision_fs == 0 {
            return Err(IrValidationError::new(
                "precision_fs",
                "scheduler precision must be non-zero",
            ));
        }
        let model = Self {
            design_name,
            precision_fs,
            waveform: parts.waveform || parts.wave_start.is_some(),
            wave_start: parts.wave_start,
            signals: parts.signals,
            net_groups: parts.net_groups,
            arrays: parts.arrays,
            containers: parts.containers,
            objects: parts.objects,
            classes: parts.classes,
            native_accesses: parts.native_accesses,
            native_types: Vec::new(),
            native_values: Vec::new(),
            pattern_types: Vec::new(),
            pattern_classes: Vec::new(),
            class_allocations: parts.class_allocations,
            virtual_interfaces: parts.virtual_interfaces,
            udp_tables: parts.udp_tables,
            udp_states: Vec::new(),
            module_paths: Vec::new(),
            switch_network: IrSwitchNetwork::default(),
            path_pulse: IrPathPulseOptions::default(),
            timing_checks: Vec::new(),
            timing_check_options: IrTimingCheckOptions::default(),
            timing_doorbell: None,
            sdf_annotations: Vec::new(),
            sdf_startup: Vec::new(),
            sdf_gate_delays: Vec::new(),
            sdf_loads: Vec::new(),
            events: parts.events,
            funcs: parts.funcs,
            assertions: parts.assertions,
            sampled_clocks: parts.sampled_clocks,
            sampled_domains: parts.sampled_domains,
            processes: parts.processes,
            init_steps: parts.init_steps,
            spawns: parts.spawns,
            final_spawns: parts.final_spawns,
            vpi_objects: Vec::new(),
            vpi_typespecs: Vec::new(),
            vpi_dynamic: IrVpiDynamic::default(),
            vpi_compile_calls: Vec::new(),
            vpi_builtin_calls: Vec::new(),
            vpi_catalog: true,
            vpi_delays_writable: false,
            vpi_delay_select: 1,
            random: IrRandomUse::default(),
            rand_problems: Vec::new(),
            free_variables: Vec::new(),
            free_sets: Vec::new(),
            covergroups: Vec::new(),
            coverage_types: Vec::new(),
            assertion_coverage: None,
            dpi_scopes: Vec::new(),
            dpi_sites: Vec::new(),
        };
        model.validate()?;
        Ok(model)
    }

    pub fn design_name(&self) -> &str {
        &self.design_name
    }
    pub fn precision_fs(&self) -> u64 {
        self.precision_fs
    }
    pub fn waveform_enabled(&self) -> bool {
        self.waveform
    }
    /// Driver-requested dumping from time zero, if any.
    pub fn wave_start(&self) -> Option<&IrWaveStart> {
        self.wave_start.as_ref()
    }
    pub fn signals(&self) -> &[IrSignal] {
        &self.signals
    }
    /// Whether `signal` is an active free checker variable, which sampled
    /// contexts read with its current value (IEEE 1800-2009 17.7.2).
    pub fn reads_current_value(&self, signal: usize) -> bool {
        !self.free_variables.is_empty() && self.free_variables.binary_search(&signal).is_ok()
    }
    pub fn free_sets(&self) -> &[IrFreeSet] {
        &self.free_sets
    }
    /// Shared combinational UDP definitions in deterministic lowering order.
    pub fn udp_tables(&self) -> &[IrUdpTable] {
        &self.udp_tables
    }
    /// Definition index of every sequential UDP instance state.
    pub fn udp_states(&self) -> &[usize] {
        &self.udp_states
    }
    /// Module path sets, one per module instance with simple paths.
    pub fn module_paths(&self) -> &[IrModulePathSet] {
        &self.module_paths
    }
    /// The design's bidirectional pass switches.
    pub fn switch_network(&self) -> &IrSwitchNetwork {
        &self.switch_network
    }
    /// Global pulse controls of the module paths.
    pub fn path_pulse(&self) -> IrPathPulseOptions {
        self.path_pulse
    }
    /// Timing check sets, one per module instance with timing checks.
    pub fn timing_checks(&self) -> &[IrTimingCheckSet] {
        &self.timing_checks
    }
    /// Global timing check controls.
    pub fn timing_check_options(&self) -> IrTimingCheckOptions {
        self.timing_check_options
    }
    /// The signal that wakes the timing process.
    pub fn timing_doorbell(&self) -> Option<usize> {
        self.timing_doorbell
    }
    /// SDF annotations, by [`IrStmt::SdfAnnotate`] index.
    pub fn sdf_annotations(&self) -> &[IrSdfAnnotation] {
        &self.sdf_annotations
    }
    /// Annotations applied before simulation starts (`--sdf`), in order.
    pub fn sdf_startup(&self) -> &[u32] {
        &self.sdf_startup
    }
    /// Writable delays of the annotated gates, by slot.
    pub fn sdf_gate_delays(&self) -> &[IrTransitionDelay] {
        &self.sdf_gate_delays
    }
    /// Load port bits with interconnect delays, by [`IrSdfTarget::Load`]
    /// index.
    pub fn sdf_loads(&self) -> &[IrSdfLoad] {
        &self.sdf_loads
    }
    pub fn net_groups(&self) -> &[IrNetGroup] {
        &self.net_groups
    }
    pub fn arrays(&self) -> &[IrArray] {
        &self.arrays
    }
    pub fn containers(&self) -> &[IrContainer] {
        &self.containers
    }
    pub fn events(&self) -> &[IrEvent] {
        &self.events
    }

    pub fn virtual_interfaces(&self) -> &[IrVirtualInterface] {
        &self.virtual_interfaces
    }
    pub fn funcs(&self) -> &[IrFunc] {
        &self.funcs
    }
    pub fn assertions(&self) -> &[IrAssertion] {
        &self.assertions
    }
    pub fn sampled_domains(&self) -> &[IrSampledDomain] {
        &self.sampled_domains
    }
    pub fn sampled_clocks(&self) -> &[IrSampledClock] {
        &self.sampled_clocks
    }
    pub fn processes(&self) -> &[IrProcess] {
        &self.processes
    }
    pub fn init_steps(&self) -> &[IrInitStep] {
        &self.init_steps
    }
    pub fn spawns(&self) -> &[String] {
        &self.spawns
    }
    pub fn final_spawns(&self) -> &[String] {
        &self.final_spawns
    }

    pub fn vpi_objects(&self) -> &[IrVpiObject] {
        &self.vpi_objects
    }

    pub fn vpi_typespecs(&self) -> &[IrVpiTypespec] {
        &self.vpi_typespecs
    }

    pub fn vpi_dynamic(&self) -> &IrVpiDynamic {
        &self.vpi_dynamic
    }

    pub fn vpi_compile_calls(&self) -> &[IrVpiCompileCall] {
        &self.vpi_compile_calls
    }

    pub fn dpi_scopes(&self) -> &[IrDpiScope] {
        &self.dpi_scopes
    }

    pub fn dpi_sites(&self) -> &[IrDpiSite] {
        &self.dpi_sites
    }

    /// Whether DPI-C calls cross a context boundary: some import is
    /// `context` or some subroutine is exported. Only then do import calls
    /// save, set and restore the call-chain state (SV 35.5.3); otherwise no
    /// foreign code can observe it.
    pub fn dpi_context_used(&self) -> bool {
        self.funcs.iter().any(|function| {
            function.dpi_export.is_some()
                || function.dpi.as_ref().is_some_and(IrDpiImport::is_context)
        })
    }

    pub fn signal(&self, idx: usize) -> &IrSignal {
        &self.signals[idx]
    }

    pub fn array(&self, idx: usize) -> &IrArray {
        &self.arrays[idx]
    }

    pub fn func(&self, idx: usize) -> &IrFunc {
        &self.funcs[idx]
    }

    pub fn net_group(&self, idx: usize) -> &IrNetGroup {
        &self.net_groups[idx]
    }

    pub fn event(&self, idx: usize) -> &IrEvent {
        &self.events[idx]
    }

    /// `(function name, spawn label)` pairs in spawn order.
    /// A spawn takes the label of the first process with its C name.
    pub fn spawn_list(&self) -> Vec<(&str, &str)> {
        // Index first-by-name once; a per-spawn search over every process
        // made this quadratic in the number of processes.
        let mut labels = std::collections::HashMap::with_capacity(self.processes.len());
        for process in &self.processes {
            labels
                .entry(process.c_name.as_str())
                .or_insert(process.label.as_str());
        }
        self.spawns
            .iter()
            .map(|f| (f.as_str(), labels.get(f.as_str()).copied().unwrap_or("")))
            .collect()
    }
}

#[cfg(test)]
mod tests;
