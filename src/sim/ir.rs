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
mod native_access;
pub use native_access::{IrNativeAccess, IrNativeAccessKind, IrClassAllocation};
mod objects;
mod validate;
pub use containers::{
    IrAssocKey, IrAssocTraversal, IrContainer, IrContainerElement, IrContainerExpr,
    IrContainerKind, IrContainerMember, IrContainerMethod, IrContainerReduction, IrContainerStmt,
    IrQueueBound, IrQueueSource, IrStreamSelector,
};
pub use objects::{
    IrArrayDimension, IrArrayQuery, IrArrayQueryKind, IrArrayQueryTarget, IrChandleExpr, IrClass,
    IrClassField, IrClassFieldType, IrDisplayArg, IrMailboxElement, IrMailboxExpr, IrMailboxTarget,
    IrMailboxValue, IrObject, IrObjectQuery, IrObjectStmt, IrObjectType, IrProcessControl,
    IrProcessExpr, IrStringExpr, IrStringInsideItem, IrVirtualInterface,
    IrVirtualInterfaceInstance, IrVirtualInterfaceMember, IrVirtualInterfaceMethod,
};

pub use validate::IrValidationError;

mod constants;
pub use constants::IrConst;
mod expressions;
pub use expressions::{
    IrBinOp, IrBitQuery, IrDynamicCast, IrEnumMember, IrEnumMethod, IrEnumQuery, IrExpr,
    IrExprKind, IrFileInput, IrFileInputTarget, IrFileReadTarget, IrMathFunc, IrMutationExpr,
    IrPlusArgTarget, IrPlusArgText, IrRandomFunc, IrRealBinOp, IrRealUnOp, IrSampledCall,
    IrSampledDomain, IrSampledFunc, IrSysFunc, IrTimeKind, IrUnOp,
};
mod lvalues;
pub use lvalues::{IrElemSel, IrInsideItem, IrLhs, IrStreamDirection, IrStreamTarget};
mod calls;
pub use calls::{IrCall, IrCallArg, IrCallExpr, IrDepth, IrVirtualCall};
mod statements;
pub use statements::{
    IrActivationTarget, IrCaseItem, IrCaseKind, IrClockingSampleMode, IrDisplayRadix, IrFileOp,
    IrMemoryRadix, IrStmt, IrStochasticStmt, IrUniquePriorityCheck, IrWaveDumpVars,
};
mod events;
pub use events::{
    IrCapture, IrCapturedBranch, IrDeferredAction, IrDelay, IrEdge, IrEvent, IrEventCapture,
    IrEventContext, IrEventRef, IrJoinKind, IrTransitionDelay, IrWaitSrc,
};
mod assertions;
pub use assertions::{
    IrAssertion, IrAssertionControlKind, IrConcurrentAssertionKind, IrImmediateAssertionKind,
    IrSequence, IrSequenceLocal, IrSequenceRange, IrSequenceTransition, IrSeverityLevel,
};
mod processes;
pub use processes::{IrPreFn, IrProcess, IrProcessKind, IrShape};
mod functions;
pub use functions::{IrDpiImport, IrFormal, IrFormalMode, IrFunc, IrLocal};
mod initialization;
pub use initialization::{IrInitPhase, IrInitStep, IrInitTarget, IrInitialization};
mod storage;
pub use storage::{IrArray, IrNetAliasBinding, IrNetGroup, IrNetKind, IrSignal};
mod vpi;
pub use vpi::{IrVpiCompileArg, IrVpiCompileCall, IrVpiObject, IrVpiObjectKind};

/// Maximum contributions stored by one generated `llg_net_t`.
pub const LLG_MAX_NET_DRIVERS: usize = 16;
/// Maximum number of arguments exposed through one generated VPI call.
pub const LLG_MAX_VPI_ARGS: usize = 256;

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
    /// At least one waveform-control system task was lowered.
    pub(in crate::sim) waveform: bool,
    pub(in crate::sim) signals: Vec<IrSignal>,
    pub(in crate::sim) net_groups: Vec<IrNetGroup>,
    pub(in crate::sim) arrays: Vec<IrArray>,
    pub(in crate::sim) containers: Vec<IrContainer>,
    pub(in crate::sim) objects: Vec<IrObject>,
    /// Nominal class layouts used by class handles and method receivers.
    pub(in crate::sim) classes: Vec<IrClass>,
    /// Typed dynamic member lvalues, resolved at each use rather than C fragments.
    pub(in crate::sim) native_accesses: Vec<IrNativeAccess>,
    pub(in crate::sim) class_allocations: Vec<IrClassAllocation>,
    /// Virtual-interface descriptors and their concrete instance bindings.
    pub(in crate::sim) virtual_interfaces: Vec<IrVirtualInterface>,
    pub(in crate::sim) events: Vec<IrEvent>,
    pub(in crate::sim) funcs: Vec<IrFunc>,
    /// Concurrent assertion instances, kept outside ordinary process IR.
    pub(in crate::sim) assertions: Vec<IrAssertion>,
    /// Explicit sampled-value clock/history domains used by system functions.
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
    /// Type-only VPI call sites run through compiletf/sizetf before the
    /// generated scheduler starts. Empty for hand-built IR fixtures.
    pub(in crate::sim) vpi_compile_calls: Vec<IrVpiCompileCall>,
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
    pub signals: Vec<IrSignal>,
    pub net_groups: Vec<IrNetGroup>,
    pub arrays: Vec<IrArray>,
    pub containers: Vec<IrContainer>,
    pub objects: Vec<IrObject>,
    pub classes: Vec<IrClass>,
    pub native_accesses: Vec<IrNativeAccess>,
    pub class_allocations: Vec<IrClassAllocation>,
    pub virtual_interfaces: Vec<IrVirtualInterface>,
    pub events: Vec<IrEvent>,
    pub funcs: Vec<IrFunc>,
    pub assertions: Vec<IrAssertion>,
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
            waveform: parts.waveform,
            signals: parts.signals,
            net_groups: parts.net_groups,
            arrays: parts.arrays,
            containers: parts.containers,
            objects: parts.objects,
            classes: parts.classes,
            native_accesses: parts.native_accesses,
            class_allocations: parts.class_allocations,
            virtual_interfaces: parts.virtual_interfaces,
            events: parts.events,
            funcs: parts.funcs,
            assertions: parts.assertions,
            sampled_domains: parts.sampled_domains,
            processes: parts.processes,
            init_steps: parts.init_steps,
            spawns: parts.spawns,
            final_spawns: parts.final_spawns,
            vpi_objects: Vec::new(),
            vpi_compile_calls: Vec::new(),
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
    pub fn signals(&self) -> &[IrSignal] {
        &self.signals
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

    pub fn vpi_compile_calls(&self) -> &[IrVpiCompileCall] {
        &self.vpi_compile_calls
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
    pub fn spawn_list(&self) -> Vec<(&str, &str)> {
        self.spawns
            .iter()
            .map(|f| {
                let label = self
                    .processes
                    .iter()
                    .find(|p| p.c_name == *f)
                    .map(|p| p.label.as_str())
                    .unwrap_or("");
                (f.as_str(), label)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
