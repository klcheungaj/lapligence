//! Owned semantic node database.
//!
//! [`Db::from_slang`] validates and projects one fully owned Slang snapshot
//! into an arena indexed by [`NodeId`]. Simulator, lint, model, and language
//! server consumers share this frontend-neutral representation. Typed side
//! tables retain array categories, packed dimensions, aggregate layouts,
//! exact constants, implicit conversions, source identity, and source text.
//! Unsupported constructs remain explicit and are rejected by the consumer
//! that requires them.

use super::slang_types::SlangTypeProjector;
use super::{
    AlwaysKind, CapturedSemanticKind, CaseKind, ConstantType, DbValidationError, Direction,
    JoinKind, NetType, ObjectType, Operation, PrimitiveType, Strength, UniquePriorityCheck,
};

use crate::core::elab::Val;
use crate::core::model::TypeInfo;
use crate::core::value::ValueData;
use crate::ffi::slang::{
    ConstantValue as SlangConstantValue, LanguageEdition, SemanticDefinitionKind,
    SemanticDriveStrength, SemanticEdgeRole, SemanticKind, SemanticNode, SemanticOperation,
    SemanticTimeScale, SemanticTimeUnit, Snapshot as SlangSnapshot, CLOCKING_BLOCK_DEFAULT,
    CLOCKING_BLOCK_GLOBAL, CLOCKING_EDGE_MASK, CLOCKING_INPUT_EDGE_SHIFT,
    CLOCKING_OUTPUT_EDGE_SHIFT, CLOCKING_VAR_OUTPUT_EDGE_SHIFT, SEMANTIC_ASSERTION_ABORT_REJECT,
    SEMANTIC_ASSERTION_ABORT_SYNC, SEMANTIC_ASSERTION_DEFERRED, SEMANTIC_ASSERTION_FINAL,
    SEMANTIC_ASSERTION_RANGE, SEMANTIC_ASSERTION_REPETITION, SEMANTIC_ASSERTION_STRONG,
    SEMANTIC_EXPR_CLOCKING_EVENT, SEMANTIC_SCOPE_CLOCKING_BLOCK, SEMANTIC_STMT_CONCURRENT_ASSERT,
    SEMANTIC_STMT_CONCURRENT_ASSUME, SEMANTIC_STMT_CONCURRENT_COVER,
    SEMANTIC_STMT_IMMEDIATE_ASSERT, SEMANTIC_STMT_IMMEDIATE_ASSUME, SEMANTIC_STMT_IMMEDIATE_COVER,
    SEMANTIC_TIMING_ONE_STEP_DELAY, SEMANTIC_VARIABLE_CLOCKING,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

/// Arena index of one [`Node`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// Stable arena index within the database that produced this ID.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One concrete packed dimension captured from an elaborated typespec.
///
/// Bounds copied from Slang's resolved type table. A missing dimension entry
/// means the dimension exists but its bounds cannot fit this legacy view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedRange {
    pub left: i128,
    pub right: i128,
}

/// Stable identity of one frontend-owned type record.
///
/// This is deliberately the captured semantic type id, not a display name or
/// a pointer into Slang.  It remains useful when two anonymous aggregates have
/// identical members but are not assignment-compatible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u64);

/// Owned nominal metadata for one class declaration or concrete generic
/// specialization.  The frontend identity is reduced to arena/type ids so
/// inheritance and construction never require retaining a Slang pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassMetadata {
    pub type_id: Option<TypeId>,
    pub base: Option<NodeId>,
    /// Frontend-owned call used to initialize the base, when the source has
    /// explicit `super.new(...)` or extends-clause arguments.
    pub base_constructor: Option<NodeId>,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_interface: bool,
}

/// Copy policy for a recursive value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueCopySemantics {
    /// Copy the value recursively, including every fixed aggregate element.
    Deep,
    /// Copy the owning handle while preserving the referenced object's
    /// identity.  Dynamic containers and opaque handles use this policy until
    /// their dedicated runtime contracts are lowered.
    Handle,
}

/// Default initialization policy for a recursive value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueDefaultSemantics {
    FourStateX,
    TwoStateZero,
    RealZero,
    EmptyString,
    NullHandle,
    Recursive,
}

/// Destruction policy for owned recursive storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueDestroySemantics {
    Trivial,
    Recursive,
    Handle,
}

/// Equality policy recorded with a recursive descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueEqualitySemantics {
    FourState,
    Real,
    String,
    Recursive,
    HandleIdentity,
    Unsupported,
}

/// The recursive shape of a frontend-owned value.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeShape {
    PackedAtom {
        ranges: Vec<PackedRange>,
    },
    Real {
        shortreal: bool,
    },
    String,
    Aggregate(AggregateLayout),
    FixedArray {
        dimensions: Vec<(i32, i32)>,
        element: Box<TypeDescriptor>,
    },
    Container {
        kind: String,
        element: Box<TypeDescriptor>,
    },
    Opaque {
        kind: String,
    },
}

/// Canonical owned type metadata used by aggregate and future activation
/// storage.  It intentionally contains no frontend references.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDescriptor {
    pub id: TypeId,
    /// Exact frontend-rendered type spelling, retained for `$typename` and
    /// diagnostics without requiring a frontend object at simulation time.
    pub name: String,
    pub info: TypeInfo,
    pub shape: TypeShape,
}

/// One declared member of an enumerated type, retained in declaration order.
///
/// Enum values are copied from Slang's resolved constants while the snapshot
/// is imported.  Consumers therefore never need to revisit the native AST to
/// implement enum queries.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumMember {
    pub name: String,
    pub value: Val,
}

/// Complete owned metadata needed by the six SystemVerilog enum methods.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumTypeMetadata {
    pub width: u32,
    pub signed: bool,
    pub two_state: bool,
    pub members: Vec<EnumMember>,
}

impl TypeDescriptor {
    pub fn copy_semantics(&self) -> ValueCopySemantics {
        match self.shape {
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => ValueCopySemantics::Handle,
            _ => ValueCopySemantics::Deep,
        }
    }

    pub fn default_semantics(&self) -> ValueDefaultSemantics {
        match self.shape {
            TypeShape::PackedAtom { .. } => {
                if self.info.kind == "bit"
                    || matches!(
                        self.info.kind.as_str(),
                        "int" | "integer" | "longint" | "byte" | "shortint" | "time"
                    )
                {
                    ValueDefaultSemantics::TwoStateZero
                } else {
                    ValueDefaultSemantics::FourStateX
                }
            }
            TypeShape::Real { .. } => ValueDefaultSemantics::RealZero,
            TypeShape::String => ValueDefaultSemantics::EmptyString,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueDefaultSemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => {
                ValueDefaultSemantics::NullHandle
            }
        }
    }

    pub fn destroy_semantics(&self) -> ValueDestroySemantics {
        match self.shape {
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueDestroySemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => ValueDestroySemantics::Handle,
            _ => ValueDestroySemantics::Trivial,
        }
    }

    pub fn equality_semantics(&self) -> ValueEqualitySemantics {
        match self.shape {
            TypeShape::PackedAtom { .. } => ValueEqualitySemantics::FourState,
            TypeShape::Real { .. } => ValueEqualitySemantics::Real,
            TypeShape::String => ValueEqualitySemantics::String,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueEqualitySemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => {
                ValueEqualitySemantics::HandleIdentity
            }
        }
    }

    pub fn fixed_size_bits(&self) -> Option<u64> {
        match &self.shape {
            TypeShape::PackedAtom { .. } => self.info.width.map(u64::from),
            TypeShape::Aggregate(layout) => match layout.kind {
                AggregateKind::PackedStruct => {
                    layout.members.iter().try_fold(0u64, |total, member| {
                        total.checked_add(member.descriptor.fixed_size_bits()?)
                    })
                }
                AggregateKind::PackedUnion => layout
                    .members
                    .iter()
                    .map(|member| member.descriptor.fixed_size_bits())
                    .try_fold(0u64, |largest, width| Some(largest.max(width?))),
                _ => None,
            },
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let count = dimensions.iter().try_fold(1u64, |total, (left, right)| {
                    let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs();
                    total.checked_mul(extent.checked_add(1)?)
                })?;
                element.fixed_size_bits()?.checked_mul(count)
            }
            _ => None,
        }
    }
}

/// Ordered packed dimensions for one elaborated declaration.
///
/// Arena identity distinguishes parameterized instances and same-named locals
/// in unnamed scopes, whose display paths can coincide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaboratedTypeRanges {
    pub declaration: NodeId,
    pub instance: String,
    pub name: String,
    pub packed_ranges: Vec<Option<PackedRange>>,
}

/// One top-level member of a packed structure or union, expressed as bit
/// positions in its containing packed value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedMember {
    pub name: String,
    pub lsb: u32,
    pub width: u32,
    pub signed: bool,
    pub two_state: bool,
    /// Effective declared packed dimensions, outermost first. Atomic types
    /// without an explicit range use their implicit `[width-1:0]` range.
    pub packed_ranges: Vec<PackedRange>,
}

/// Representation category of a captured SystemVerilog structure or union.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateKind {
    PackedStruct,
    PackedUnion,
    UnpackedStruct,
    UnpackedUnion,
    TaggedUnion,
}

/// One declared member of an unpacked aggregate.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregateMember {
    pub name: String,
    pub ty: TypeInfo,
    pub two_state: bool,
    pub packed_ranges: Vec<PackedRange>,
    /// Nested structure/union layout when this member is itself aggregate.
    pub aggregate: Option<Box<AggregateLayout>>,
    /// Complete recursive member type, including fixed unpacked arrays and
    /// non-integral leaves which do not have a packed width.
    pub descriptor: TypeDescriptor,
}

impl AggregateMember {
    /// Return the canonical nested aggregate description, when this member is
    /// itself a structure or union.
    pub fn aggregate_layout(&self) -> Option<&AggregateLayout> {
        self.aggregate.as_deref().or(match &self.descriptor.shape {
            TypeShape::Aggregate(layout) => Some(layout),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AggregateLayout {
    pub kind: AggregateKind,
    /// Database-local identity of Slang's canonical aggregate type. Equal
    /// identities prove assignment compatibility; member shape alone does not.
    pub type_identity: Option<String>,
    pub type_id: Option<TypeId>,
    pub members: Vec<AggregateMember>,
}

/// Exact owned metadata for a type key in an assignment pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentPatternKeyType {
    /// Canonical frontend type identity.  Width, signedness, and state domain
    /// are not sufficient to decide whether a nominal aggregate/type key
    /// matches a member.
    pub type_id: TypeId,
    pub ty: TypeInfo,
    pub two_state: bool,
    pub packed_ranges: Vec<PackedRange>,
}

impl ElaboratedTypeRanges {
    pub fn instance(&self) -> &str {
        &self.instance
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn packed_ranges(&self) -> &[Option<PackedRange>] {
        &self.packed_ranges
    }
}

/// Failure to construct a structurally usable owned semantic database.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbError {
    /// The owned Slang snapshot was inconsistent or incomplete.
    InvalidSnapshot(String),
    /// The captured owned graph violated an internal database invariant.
    InvalidDatabase(DbValidationError),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSnapshot(detail) => f.write_str(detail),
            Self::InvalidDatabase(error) => error.fmt(f),
        }
    }
}

impl Error for DbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidDatabase(error) => Some(error),
            Self::InvalidSnapshot(_) => None,
        }
    }
}

impl From<String> for DbError {
    fn from(detail: String) -> Self {
        Self::InvalidSnapshot(detail)
    }
}

#[derive(Debug)]
pub struct Db {
    nodes: Vec<Node>,
    /// Complete language policy selected for the owned frontend snapshot.
    edition: LanguageEdition,
    overridden_parameters: HashSet<NodeId>,
    /// Native semantic categories retained for coverage checks when the
    /// frontend-neutral [`NodeKind`] intentionally has no direct variant.
    semantic_kinds: Vec<CapturedSemanticKind>,
    /// Native detail text retained alongside [`semantic_kinds`] for
    /// source-located diagnostics about otherwise unsupported nodes.
    semantic_details: Vec<String>,
    /// Elaborated module-instance/definition IDs whose definition is a
    /// SystemVerilog program.  Program identity is kept as owned semantic
    /// metadata rather than inferred from names or source text so simulator
    /// scheduling can distinguish it after the Slang snapshot is released.
    program_instances: HashSet<NodeId>,
    tops: Vec<NodeId>,
    flat_modules: Vec<NodeId>,
    packages: Vec<NodeId>,
    classes: Vec<NodeId>,
    /// Class inheritance and nominal type metadata keyed by class node.
    class_metadata: HashMap<NodeId, ClassMetadata>,
    design_name: String,
    /// Unpacked-array dimension/initializer metadata, keyed by each
    /// [`NodeKind::Array`] arena node (see [`ArrayMeta`]).
    arrays: HashMap<NodeId, ArrayMeta>,
    /// Unpacked-array metadata for named-event declarations. Event arrays keep
    /// their declaration identity as [`NodeKind::NamedEvent`] while this side
    /// table records the index shape needed by the simulator.
    event_arrays: HashMap<NodeId, ArrayMeta>,
    /// Canonical owner/path for array-select expressions whose frontend base
    /// is a detached synthetic array node (for example a member array inside
    /// an unpacked aggregate).  Keeping this identity in the owned snapshot
    /// avoids resolving equal display names at lowering time.
    array_select_paths: HashMap<NodeId, (NodeId, Vec<String>)>,
    vars_init: HashMap<NodeId, NodeId>,
    /// Propagation delays declared on net symbols, kept separate from the
    /// synthetic declaration-assignment driver delay.
    net_delays: HashMap<NodeId, DriverDelay>,
    var_lifetimes: HashMap<NodeId, VariableLifetime>,
    var_lifetime_qualifiers: HashMap<NodeId, VariableLifetimeQualifier>,
    method_calls_with_clause: HashSet<NodeId>,
    /// Method-call node → the frontend-owned iterator declaration used by its
    /// `with` expression.  Slang visits the expression itself as a structural
    /// child, but the implicit iterator variable is not an argument edge.
    method_call_iterators: HashMap<NodeId, NodeId>,
    /// Top-level packed struct/union layouts keyed by the declared object.
    packed_members: HashMap<NodeId, Vec<PackedMember>>,
    /// Structure/union category and members keyed by the declared object.
    aggregate_layouts: HashMap<NodeId, AggregateLayout>,
    /// Complete recursive type descriptors keyed by the declared object.
    type_descriptors: HashMap<NodeId, TypeDescriptor>,
    /// Ordered enum members keyed by Slang's canonical type identity.
    enum_types: HashMap<TypeId, EnumTypeMetadata>,
    /// Ordered ranges of multidimensional packed declarations.
    packed_dimensions: HashMap<NodeId, Vec<PackedRange>>,
    /// True for declarations whose complete packed type has a two-state base.
    two_state_types: HashSet<NodeId>,
    /// Clocking block declarations and their resolved clock events/skews.
    clocking_blocks: HashMap<NodeId, ClockingBlockInfo>,
    /// Clocking block variables and the source signal each samples.
    clocking_vars: HashMap<NodeId, ClockingVarInfo>,
    /// Direction of each captured modport port. This is kept separately from
    /// `NodeKind::ModPort` so the frontend-neutral node shape remains stable.
    modport_directions: HashMap<NodeId, Direction>,
    /// Statically initialized virtual-interface variables and their concrete
    /// interface instances. Runtime reassignment remains outside this map.
    virtual_interface_targets: HashMap<NodeId, NodeId>,
    /// DPI-C import contracts copied from the frontend syntax/flags.  This is
    /// a side table so synthetic test nodes and the existing NodeKind ABI do
    /// not need a lossy placeholder field.
    dpi_imports: HashMap<NodeId, DpiImportInfo>,
    /// Nets declared implicitly by Slang's semantic analysis.
    implicit_nets: HashSet<NodeId>,
    /// Context conversions inserted by Slang rather than written as casts.
    implicit_conversions: HashSet<NodeId>,
    /// Exact admitted source buffers keyed by their frontend file name.
    source_files: HashMap<String, String>,
    elaborated_type_ranges: Vec<ElaboratedTypeRanges>,
}

/// Owned DPI-C declaration metadata.  The C linkage spelling is copied from
/// the import declaration (or falls back to the HDL name when omitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpiImportInfo {
    pub c_name: String,
    pub context: bool,
    pub pure: bool,
}

/// Unpacked-array metadata captured at build time, kept out of the
/// [`NodeKind::Array`] variant so `core::model` (which binds the variant's
/// `ty` field) does not have to change.
#[derive(Debug)]
pub struct ArrayMeta {
    pub kind: ArrayKind,
    pub dims: Vec<Option<(i32, i32)>>,
    pub init: Option<NodeId>,
    /// Captured subtype for an unpacked net array (including reg-shaped
    /// arrays); `None` for variable-shaped arrays or missing element metadata.
    pub net_type: Option<NetType>,
}

/// Runtime storage category for an unpacked array declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayKind {
    Static,
    Dynamic,
    Associative(AssociativeIndex),
    /// Maximum element count (`N + 1` for a `[$:N]` declaration), or `None`
    /// for an unbounded `[$]` queue.
    Queue {
        maximum_elements: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssociativeIndex {
    Wildcard,
    Integral {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    String,
    Unsupported(String),
}

impl ArrayMeta {
    pub fn kind(&self) -> &ArrayKind {
        &self.kind
    }
    pub fn dimensions(&self) -> &[Option<(i32, i32)>] {
        &self.dims
    }

    pub fn initializer(&self) -> Option<NodeId> {
        self.init
    }

    pub fn net_type(&self) -> Option<NetType> {
        self.net_type
    }
}

#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    /// Captured child objects, in capture order.
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub name: String,
    pub full_name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Node {
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub const fn line(&self) -> u32 {
        self.line
    }

    pub const fn column(&self) -> u32 {
        self.col
    }

    pub const fn end_line(&self) -> u32 {
        self.end_line
    }

    pub const fn end_column(&self) -> u32 {
        self.end_col
    }
}

/// What a captured node is.
#[derive(Debug)]
pub enum NodeKind {
    ModuleInst {
        def_name: String,
        is_top: bool,
        /// `true` when the instance is an interface (`interface_inst`) rather
        /// than a module.
        is_interface: bool,
        timeunit: i32,
        timeprecision: i32,
    },
    Package,
    ClassDef,
    Port {
        direction: Direction,
        ty: TypeInfo,
        /// Strength endpoints declared on an output/inout port.  Keep these
        /// on the owned node so collapsed net groups do not lose the port's
        /// contribution when the frontend binding is projected away.
        strength0: Strength,
        strength1: Strength,
        high: Option<NodeId>,
        low: Option<NodeId>,
        high_expr: Option<NodeId>,
        high_present: bool,
        high_open: bool,
    },
    ModPort,
    IoDecl {
        direction: Direction,
        expr: Option<NodeId>,
    },
    IfaceConn {
        /// Arena node of the actual interface instance being connected.
        actual: NodeId,
        /// Connected modport name, or `""` for a bare interface port.
        modport: String,
    },
    Net {
        ty: TypeInfo,
        net_type: NetType,
        /// Drive-strength endpoints declared on the net, used when the net
        /// is exposed through an output port or collapsed structural path.
        strength0: Strength,
        strength1: Strength,
    },
    /// A structural SystemVerilog `alias` declaration. The expressions are
    /// retained in source order so lowering can validate and merge their
    /// canonical net identities before executable children are collected.
    NetAlias {
        nets: Vec<NodeId>,
    },
    Var {
        ty: TypeInfo,
    },
    /// Elaboration-only generate-loop variable; never runtime storage.
    Genvar {
        ty: TypeInfo,
    },
    Array {
        ty: TypeInfo,
    },
    NamedEvent,
    Param {
        ty: TypeInfo,
        value: Option<Val>,
        local: bool,
    },
    ParamAssign {
        overridden: bool,
    },
    /// Slang's container for an array of elaborated instances. Concrete
    /// instance entries are normalized into the enclosing instance hierarchy.
    InstanceArray,
    GenScopeArray,
    GenScope,
    Process {
        kind: ProcessKind,
    },
    ContAssign {
        net_decl: bool,
        delay: Option<DriverDelay>,
        /// Drive strengths retained so consumers can reject unsupported
        /// strength-aware resolution instead of silently treating it as
        /// equal-strength.
        strength0: Strength,
        strength1: Strength,
    },
    Gate {
        class: PrimClass,
        prim_type: PrimitiveType,
        strength0: Strength,
        strength1: Strength,
        delay: Option<DriverDelay>,
        terms: Vec<GateTerm>,
    },
    Stmt(StmtKind),
    AssertionExpr(AssertionExprKind),
    Expr(ExprKind),
    SysCall {
        name: String,
    },
    /// Built-in object method. `receiver` is authoritative; structural
    /// children retain captured receiver, argument, and with-clause expressions
    /// without a positional guarantee.
    MethodCall {
        name: String,
        receiver: Option<NodeId>,
        /// Arena node of the resolved method callee, when Slang exposed it.
        /// Keeping this identity separate from the receiver lets simulator
        /// lowering bind class methods without reconstructing a name lookup.
        callee: Option<NodeId>,
    },
    FuncCall {
        name: String,
        /// `true` for a `task_call` (statement), `false` for a `func_call`.
        is_task: bool,
        /// `true` when the call uses the `super` qualifier and must bind to
        /// the declaring base implementation.
        is_super: bool,
        /// Arena node of the callee [`NodeKind::FuncTask`] (the per-instance
        /// clone), when it was already captured when the call site was walked.
        callee: Option<NodeId>,
    },
    FuncTask {
        is_task: bool,
        automatic: bool,
        /// `true` for a class method declared with the `static` qualifier.
        is_static: bool,
        /// `true` when the method participates in virtual dispatch.
        is_virtual: bool,
        /// `true` for a pure virtual method with no implementation.
        is_pure: bool,
        /// `true` when a virtual method forbids overrides in derived classes.
        is_final: bool,
        /// `true` for a class constructor (`new`).
        is_constructor: bool,
        /// Return type, `None` for void functions and tasks.
        ret: Option<TypeInfo>,
        /// Exact executable body attached by Slang.
        body: Option<NodeId>,
    },
    FuncArg {
        direction: Direction,
        ty: TypeInfo,
        default: Option<NodeId>,
        /// `true` for a `const ref` formal.
        const_ref: bool,
        /// `true` for a `ref static` formal.
        ref_static: bool,
    },
    EnumConst {
        value: Option<Val>,
    },
    /// Anything the walk does not model explicitly (never fails the build).
    Other,
}

#[derive(Debug)]
pub enum ProcessKind {
    Always { always_type: AlwaysKind },
    Initial,
    Final,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimClass {
    Gate,
    Switch,
    Udp,
    Array,
}

/// One terminal of a [`NodeKind::Gate`].
#[derive(Clone, Debug)]
pub struct GateTerm {
    pub direction: Direction,
    pub term_index: i32,
    pub expr: NodeId,
}

impl GateTerm {
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    pub const fn term_index(&self) -> i32 {
        self.term_index
    }

    pub const fn expression(&self) -> NodeId {
        self.expr
    }
}

/// One `case` item: the item expressions plus the (optional) body statement.
#[derive(Debug)]
pub struct CaseItem {
    pub exprs: Vec<NodeId>,
    pub body: Option<NodeId>,
}

impl CaseItem {
    pub fn expressions(&self) -> &[NodeId] {
        &self.exprs
    }

    pub const fn body(&self) -> Option<NodeId> {
        self.body
    }
}

/// Kind of a captured statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImmediateAssertionKind {
    Assert,
    Assume,
    Cover,
}

/// Kind of a concurrent assertion declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcurrentAssertionKind {
    Assert,
    Assume,
    Cover,
}

/// Operators in the owned assertion-expression graph.  Keeping these
/// separate from ordinary expression operators prevents a property operator
/// from being mistaken for a four-state value operation during lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionUnaryOp {
    Not,
    NextTime,
    SNextTime,
    Always,
    SAlways,
    Eventually,
    SEventually,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionBinaryOp {
    And,
    Or,
    Intersect,
    Throughout,
    Within,
    Iff,
    Until,
    SUntil,
    UntilWith,
    SUntilWith,
    Implies,
    OverlappedImplication,
    NonOverlappedImplication,
    OverlappedFollowedBy,
    NonOverlappedFollowedBy,
}

#[derive(Clone, Debug)]
pub struct AssertionCaseItem {
    pub expressions: Vec<NodeId>,
    pub body: NodeId,
}

/// One formal-to-actual mapping retained for a named sequence/property
/// instance. The assertion body is still owned separately, so lowering can
/// reject unsupported instances without discarding the binding identity.
#[derive(Clone, Debug)]
pub struct AssertionBinding {
    pub formal: NodeId,
    pub actual: NodeId,
}

/// Owned property/sequence node.  Unsupported forms remain represented with
/// all child identities intact and are rejected by simulator lowering.
#[derive(Debug)]
pub enum AssertionExprKind {
    Invalid {
        child: Option<NodeId>,
    },
    Simple {
        expr: NodeId,
        repeated: bool,
    },
    SequenceConcat {
        elements: Vec<NodeId>,
    },
    SequenceWithMatch {
        expr: NodeId,
        match_items: Vec<NodeId>,
        repeated: bool,
    },
    Unary {
        op: AssertionUnaryOp,
        expr: NodeId,
        ranged: bool,
    },
    Binary {
        op: AssertionBinaryOp,
        left: NodeId,
        right: NodeId,
    },
    FirstMatch {
        sequence: NodeId,
        match_items: Vec<NodeId>,
    },
    Clocking {
        control: NodeId,
        signal: NodeId,
        posedge: bool,
        expr: NodeId,
    },
    StrongWeak {
        expr: NodeId,
        strong: bool,
    },
    Abort {
        condition: NodeId,
        expr: NodeId,
        reject: bool,
        sync: bool,
    },
    Conditional {
        condition: NodeId,
        if_expr: NodeId,
        else_expr: Option<NodeId>,
    },
    Case {
        expr: NodeId,
        items: Vec<AssertionCaseItem>,
        default_case: Option<NodeId>,
    },
    DisableIff {
        condition: NodeId,
        expr: NodeId,
    },
}

impl AssertionExprKind {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Invalid { child } => child.iter().for_each(|id| nodes.push(*id)),
            Self::Simple { expr, .. } => nodes.push(*expr),
            Self::SequenceConcat { elements } => nodes.extend(elements),
            Self::SequenceWithMatch {
                expr, match_items, ..
            } => {
                nodes.push(*expr);
                nodes.extend(match_items);
            }
            Self::Unary { expr, .. } | Self::StrongWeak { expr, .. } => nodes.push(*expr),
            Self::Binary { left, right, .. } => nodes.extend([*left, *right]),
            Self::FirstMatch {
                sequence,
                match_items,
            } => {
                nodes.push(*sequence);
                nodes.extend(match_items);
            }
            Self::Clocking {
                control,
                signal,
                expr,
                ..
            } => nodes.extend([*control, *signal, *expr]),
            Self::Abort {
                condition, expr, ..
            } => nodes.extend([*condition, *expr]),
            Self::Conditional {
                condition,
                if_expr,
                else_expr,
            } => {
                nodes.extend([*condition, *if_expr]);
                else_expr.iter().for_each(|id| nodes.push(*id));
            }
            Self::Case {
                expr,
                items,
                default_case,
            } => {
                nodes.push(*expr);
                for item in items {
                    nodes.extend(&item.expressions);
                    nodes.push(item.body);
                }
                default_case.iter().for_each(|id| nodes.push(*id));
            }
            Self::DisableIff { condition, expr } => nodes.extend([*condition, *expr]),
        }
    }
}

#[derive(Debug)]
pub enum StmtKind {
    Begin,
    /// An immediate assertion with owned condition and action branches.
    /// Deferred/final metadata is retained so unsupported forms fail closed
    /// during simulator lowering instead of becoming ordinary assertions.
    ImmediateAssertion {
        kind: ImmediateAssertionKind,
        cond: NodeId,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
        label: String,
        deferred: bool,
        is_final: bool,
    },
    /// A concurrent property assertion. The property graph is separate from
    /// ordinary statement/expression IR so sampled evaluation retains its
    /// clock, disable, attempt, and declaration identity.
    ConcurrentAssertion {
        kind: ConcurrentAssertionKind,
        property: NodeId,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
        label: String,
    },
    IfElse {
        cond: NodeId,
        check: UniquePriorityCheck,
    },
    Assign {
        blocking: bool,
        op: Operation,
        /// Intra-assignment control (`a = #5 b;`, `a <= #5 b;`, and
        /// event/repeat forms) — see
        /// [`IntraControl`].  `None` when the assignment has none.
        delay: Option<IntraControl>,
    },
    Case {
        case_type: CaseKind,
        check: UniquePriorityCheck,
        items: Vec<CaseItem>,
    },
    For {
        /// Variables declared in the initializer (`for (int i = ...; ...)`).
        vars: Vec<NodeId>,
        init: Vec<NodeId>,
        cond: NodeId,
        incr: Vec<NodeId>,
        body: NodeId,
    },
    While {
        cond: NodeId,
        body: NodeId,
    },
    /// SystemVerilog post-test loop: execute `body`, then repeat while
    /// `cond` is true.
    DoWhile {
        cond: NodeId,
        body: NodeId,
    },
    Repeat {
        cond: NodeId,
        body: NodeId,
    },
    Forever {
        body: NodeId,
    },
    EventControl {
        specs: Vec<EventSpec>,
        /// `true` for `@*` / `always_comb` with no explicit sensitivity.
        implicit: bool,
        body: Option<NodeId>,
    },
    DelayControl {
        delay: NodeId,
    },
    EventTrigger {
        blocking: bool,
        target: Option<NodeId>,
        timing: Option<EventTriggerTiming>,
    },
    /// `wait (cond) stmt` — suspend until `cond` is true, then run the body.
    /// The (optional) body statement is captured as a child node.
    Wait {
        cond: NodeId,
    },
    /// `wait_order (...) action else failure` — suspend until canonical event
    /// objects arrive in order, retaining both action statements.
    WaitOrder {
        events: Vec<NodeId>,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
    },
    /// `force lhs = rhs` — force a net/var until released or deassigned.
    Force {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// `release lhs` — cancel a procedural force on `lhs`.
    Release {
        lhs: NodeId,
    },
    /// `deassign lhs` — cancel a procedural continuous assignment on `lhs`.
    Deassign {
        lhs: NodeId,
    },
    ProcContAssign {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// A declaration statement at its executable lexical position. The
    /// declaration node owns its initializer through [`Db::var_initializer`].
    VariableDecl {
        declaration: NodeId,
    },
    Empty,
    Return {
        value: Option<NodeId>,
    },
    Fork {
        /// Resolved declaration identity for a named fork scope. Anonymous
        /// fork statements carry no target.
        target: Option<NodeId>,
        join_kind: JoinKind,
        branches: Vec<NodeId>,
    },
    /// `wait fork;` — suspend until every live fork group of the current
    /// process has completed.  Atomic: no children.
    WaitFork,
    /// `disable fork;` — kill every descendant of the current process.
    /// Atomic: no children.
    DisableFork,
    Disable {
        target: Option<NodeId>,
    },
    /// `break;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Break,
    /// `continue;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Continue,
    /// `foreach (array[index, ...]) body` loop. The array target is resolved
    /// against an already-captured declaration; iterator variables belong to
    /// this statement's lexical scope. `None` entries preserve omitted
    /// dimensions, including omitted trailing dimensions.
    Foreach {
        array: Option<NodeId>,
        vars: Vec<Option<NodeId>>,
        body: NodeId,
    },
    Unsupported {
        object_type: ObjectType,
    },
}

/// Ordered propagation-delay expressions on a continuous assignment or primitive.
#[derive(Clone, Copy, Debug)]
pub enum DriverDelay {
    /// One expression supplies every transition delay.
    Single(NodeId),
    /// Separate rise and fall expressions; turn-off is their minimum.
    RiseFall(NodeId, NodeId),
    /// Separate rise, fall and turn-off expressions, in that order.
    RiseFallTurnOff(NodeId, NodeId, NodeId),
}

#[derive(Debug)]
pub enum IntraControl {
    /// Delay expression evaluated in the assignment's owning scope.
    Delay(NodeId),
    /// One event control (`@(posedge a or ev)`) retained as owned event specs.
    Event {
        control: NodeId,
        specs: Vec<EventSpec>,
        implicit: bool,
    },
    /// A repeat event control (`repeat (n) @(...)`).  The nested control is
    /// retained so lowering never has to recover syntax or source text.
    Repeat {
        control: NodeId,
        count: NodeId,
        event: Box<IntraControl>,
    },
    /// A timing form known to the frontend but not yet executable by the
    /// simulator.  Keeping its identity gives consumers a source-located
    /// diagnostic instead of silently treating it as an untimed assignment.
    Unsupported { control: NodeId },
}

impl IntraControl {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Delay(delay) => nodes.push(*delay),
            Self::Event { control, specs, .. } => {
                nodes.push(*control);
                for spec in specs {
                    spec.referenced_nodes(nodes);
                }
            }
            Self::Repeat {
                control,
                count,
                event,
            } => {
                nodes.extend([*control, *count]);
                event.referenced_nodes(nodes);
            }
            Self::Unsupported { control } => nodes.push(*control),
        }
    }
}

/// Timing attached to a nonblocking named-event trigger (`->> timing ev`).
///
/// The timing control remains an owned semantic value until simulator
/// lowering.  Unsupported timing nodes are retained so a consumer can issue
/// a source-located rejection instead of silently treating them as an
/// immediate trigger.
#[derive(Debug)]
pub enum EventTriggerTiming {
    Delay {
        control: NodeId,
        expression: NodeId,
    },
    Event {
        control: NodeId,
        specs: Vec<EventSpec>,
        implicit: bool,
    },
    Repeat {
        control: NodeId,
        count: NodeId,
        event: Box<EventTriggerTiming>,
    },
    Unsupported {
        control: NodeId,
    },
}

impl EventTriggerTiming {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Delay {
                control,
                expression,
            } => nodes.extend([*control, *expression]),
            Self::Event { control, specs, .. } => {
                nodes.push(*control);
                for spec in specs {
                    spec.referenced_nodes(nodes);
                }
            }
            Self::Repeat {
                control,
                count,
                event,
            } => {
                nodes.extend([*control, *count]);
                event.referenced_nodes(nodes);
            }
            Self::Unsupported { control } => nodes.push(*control),
        }
    }
}

/// One sensitivity entry of an event control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSpec {
    /// An event whose qualifier is sampled when its source triggers.
    Qualified {
        event: Box<EventSpec>,
        condition: NodeId,
    },
    Edge {
        sig: NodeId,
        posedge: bool,
    },
    AnyChange {
        sig: NodeId,
    },
    /// A named event (`@(ev)`) — the value is the owned event expression. It
    /// remains an expression node so array selects and hierarchical paths keep
    /// their declaration identity and indices until simulator lowering.
    Named(NodeId),
}

/// Edge selector retained for clocking block input/output skews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockingEdge {
    None,
    Posedge,
    Negedge,
    BothEdges,
}

/// Owned timing metadata for one clocking block skew. `delay` identifies the
/// captured timing control while `delay_expression` identifies its scalar
/// delay expression when the control is a regular `#` delay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingSkew {
    pub edge: ClockingEdge,
    pub delay: Option<NodeId>,
    pub delay_expression: Option<NodeId>,
}

/// Owned declaration and event metadata for one clocking block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingBlockInfo {
    pub event: NodeId,
    pub event_specs: Vec<EventSpec>,
    pub event_implicit: bool,
    pub is_default: bool,
    pub is_global: bool,
    pub default_input: ClockingSkew,
    pub default_output: ClockingSkew,
}

/// Owned source and skew metadata for one clocking block variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingVarInfo {
    pub block: NodeId,
    pub source: NodeId,
    pub direction: Direction,
    pub input: ClockingSkew,
    pub output: ClockingSkew,
}

impl EventSpec {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Qualified { event, condition } => {
                event.referenced_nodes(nodes);
                nodes.push(*condition);
            }
            Self::Edge { sig, .. } | Self::AnyChange { sig } | Self::Named(sig) => nodes.push(*sig),
        }
    }
}

/// Kind of a captured expression.
#[derive(Debug)]
pub enum ExprKind {
    /// A non-value symbol used as scope/interface metadata, never a signal read.
    /// Consumers must validate the use site before treating it as elaboration-only.
    ScopeRef {
        target: NodeId,
    },
    Constant {
        value: ValueData,
        size: i32,
        const_type: ConstantType,
        source: ConstantSource,
        time_scale: Option<TimeLiteralScale>,
    },
    Operation {
        op: Operation,
        reordered: bool,
        /// Whether this operation is the assignment-expression form of the
        /// operation (including compound assignments). The frontend uses the
        /// same semantic operation for `+` and `+=`; retaining the source
        /// operation subkind keeps that distinction in the owned DB.
        assignment: bool,
        operands: Vec<NodeId>,
    },
    /// A streaming concatenation with Slang's resolved slice size and exact
    /// per-stream selector relationships.
    Streaming {
        direction: StreamingDirection,
        slice_size: u64,
        streams: Vec<StreamOperand>,
    },
    /// One keyed operand inside an assignment pattern (`'{member: value}`).
    TaggedPattern {
        key: Option<String>,
        key_type: Option<AssignmentPatternKeyType>,
        value: Option<NodeId>,
    },
    /// `'(type)(expr)` cast — target type resolved at build time.
    Cast {
        operand: NodeId,
        ty: TypeInfo,
        /// A numeric size cast (`N'(expr)`), whose result keeps the operand's
        /// signedness rather than taking it from an integer typespec.
        size_cast: bool,
        size_cast_expr: Option<String>,
        /// False when source/decompile provenance was unavailable and the
        /// integer typespec is therefore ambiguous.
        cast_kind_known: bool,
        /// Slang propagated this context conversion into its operand. The
        /// target signedness therefore participates in width extension.
        propagated: bool,
        /// State domain of the complete target type, including aggregate and
        /// enum base types.
        two_state: bool,
    },
    Ref {
        target: Option<NodeId>,
    },
    /// A type-only expression admitted by an unevaluated system-function
    /// argument (for example `$bits(int)` or `$typename(my_t)`).
    DataType,
    /// The SystemVerilog unbounded literal `$`, retained independently from
    /// ordinary constants so `$isunbounded` does not evaluate its argument.
    Unbounded,
    BitSelect {
        base: NodeId,
        index: NodeId,
    },
    PartSelect {
        base: NodeId,
        left: NodeId,
        right: NodeId,
    },
    IndexedPartSelect {
        base: NodeId,
        base_expr: NodeId,
        width_expr: NodeId,
        neg: bool,
    },
    ArraySelect {
        base: NodeId,
        indices: Vec<NodeId>,
    },
    HierPath {
        parts: Vec<String>,
        refs: Vec<Option<NodeId>>,
    },
    /// Dynamic-array construction (`new[size]`), with an optional source
    /// array whose elements initialize the newly allocated array.
    NewArray {
        size: NodeId,
        initializer: Option<NodeId>,
    },
    /// Class-object construction (`new(...)`). The class name is retained
    /// from Slang's resolved expression type; the constructor call is the
    /// owned initializer edge when one exists.
    NewClass {
        class_name: Option<String>,
        /// Canonical class type identity; names are insufficient for generic
        /// specializations that share one source spelling.
        class_type: Option<TypeId>,
        constructor: Option<NodeId>,
        /// `true` for a `super.new(...)` expression.  It invokes the base
        /// implementation without allocating another object.
        is_super_class: bool,
    },
    AssertionInstance {
        target: NodeId,
        body: NodeId,
        bindings: Vec<AssertionBinding>,
    },
    /// A direct signal event used as an explicit sampled-value clock. Complex
    /// event lists and named events remain `Other` and fail closed in lowering.
    ClockingEvent {
        signal: NodeId,
        posedge: bool,
        gate: Option<NodeId>,
    },
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamingDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOperand {
    pub value: NodeId,
    /// Typed index or range expression attached by a stream `with` clause.
    pub with_expr: Option<NodeId>,
}

/// Owned source provenance for a captured constant.
#[derive(Debug)]
pub enum ConstantSource {
    /// Source is unnecessary (non-unsigned constant) or the frontend supplied
    /// no usable location.
    NotCaptured,
    /// Exact, bounded, single-line source span.
    Exact(String),
    /// A source location existed but bounded capture could not safely read it.
    Unavailable,
}

/// Owning scope's base time unit carried by a SystemVerilog time literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeLiteralScale {
    pub unit: TimeUnit,
    pub magnitude: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
    Picoseconds,
    Femtoseconds,
}

/// Explicit variable-lifetime provenance recovered from admitted source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableLifetimeQualifier {
    None,
    Static,
    Automatic,
    Ambiguous,
    Unavailable,
}

/// Effective variable lifetime resolved by semantic analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableLifetime {
    Static,
    Automatic,
    /// The native snapshot contains only an incomplete declaration placeholder.
    Unavailable,
}

fn semantic_edges<'a>(
    snapshot: &'a SlangSnapshot,
    node: &SemanticNode,
) -> Result<&'a [crate::ffi::slang::SemanticEdge], DbError> {
    let start = usize::try_from(node.edge_start)
        .map_err(|_| DbError::InvalidSnapshot("semantic edge start is too large".to_owned()))?;
    let count = usize::try_from(node.edge_count)
        .map_err(|_| DbError::InvalidSnapshot("semantic edge count is too large".to_owned()))?;
    let end = start
        .checked_add(count)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic edge window overflowed".to_owned()))?;
    snapshot
        .semantic_edges
        .get(start..end)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic edge window is invalid".to_owned()))
}

fn semantic_id(ids: &HashMap<u64, NodeId>, id: u64) -> Result<NodeId, DbError> {
    ids.get(&id)
        .copied()
        .ok_or_else(|| DbError::InvalidSnapshot(format!("unknown semantic node id {id}")))
}

fn canonical_reference_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    target_id: u64,
) -> Result<NodeId, DbError> {
    let target = semantic_id(ids, target_id)?;
    let semantic = snapshot
        .semantic_nodes
        .get(target.index())
        .ok_or_else(|| DbError::InvalidSnapshot("semantic reference target is missing".into()))?;
    let edges = semantic_edges(snapshot, semantic)?;
    if let Some(owner) = edge_target(ids, edges, SemanticEdgeRole::ReturnOwner)? {
        return Ok(owner);
    }
    if semantic.kind == SemanticKind::Modport {
        if let Some(internal) = semantic
            .target_id
            .map(|id| semantic_id(ids, id))
            .transpose()?
        {
            return Ok(internal);
        }
    }
    Ok(target)
}

fn edge_target(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Option<NodeId>, DbError> {
    edges
        .iter()
        .find(|edge| edge.role == role)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()
}

fn edge_target_at(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
    index: u32,
) -> Result<Option<NodeId>, DbError> {
    edges
        .iter()
        .find(|edge| edge.role == role && edge.index == index)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()
}

fn edge_targets(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Vec<NodeId>, DbError> {
    edges
        .iter()
        .filter(|edge| edge.role == role)
        .map(|edge| semantic_id(ids, edge.target_id))
        .collect()
}

fn resolved_edge_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Option<NodeId>, DbError> {
    let Some(edge) = edges.iter().find(|edge| edge.role == role) else {
        return Ok(None);
    };
    let semantic =
        snapshot
            .semantic_nodes
            .get(usize::try_from(edge.target_id).map_err(|_| {
                DbError::InvalidSnapshot("semantic edge target is too large".into())
            })?)
            .ok_or_else(|| DbError::InvalidSnapshot("semantic edge target is missing".into()))?;
    semantic
        .target_id
        .map(|id| semantic_id(ids, id))
        .transpose()?
        .map_or_else(
            || {
                matches!(
                    semantic.kind,
                    SemanticKind::Instance
                        | SemanticKind::Port
                        | SemanticKind::Net
                        | SemanticKind::Variable
                        | SemanticKind::Array
                        | SemanticKind::NamedEvent
                        | SemanticKind::Parameter
                        | SemanticKind::Subroutine
                        | SemanticKind::EnumConstant
                        | SemanticKind::InterfaceConnection
                )
                .then(|| semantic_id(ids, semantic.id))
                .transpose()
            },
            |target| Ok(Some(target)),
        )
}

fn is_array_semantic(snapshot: &SlangSnapshot, id: NodeId) -> bool {
    let Some(type_id) = snapshot
        .semantic_nodes
        .get(id.index())
        .and_then(|node| node.type_id)
    else {
        return false;
    };
    snapshot
        .types
        .iter()
        .find(|ty| ty.id == type_id)
        .is_some_and(|ty| {
            matches!(
                ty.kind,
                crate::ffi::slang::TypeKind::FixedUnpackedArray
                    | crate::ffi::slang::TypeKind::DynamicArray
                    | crate::ffi::slang::TypeKind::AssociativeArray
                    | crate::ffi::slang::TypeKind::Queue
            )
        })
}

fn array_select_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
    depth: usize,
) -> Result<Option<(NodeId, Vec<NodeId>)>, DbError> {
    if depth > snapshot.semantic_nodes.len() || node.subkind != 73 {
        return Ok(None);
    }
    let edges = semantic_edges(snapshot, node)?;
    let index = edge_target(ids, edges, SemanticEdgeRole::Index)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select index is missing".into()))?;
    let raw_base = edge_target(ids, edges, SemanticEdgeRole::Base)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select base is missing".into()))?;
    let base_semantic = &snapshot.semantic_nodes[raw_base.index()];
    if let Some((base, mut indices)) =
        array_select_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
    {
        indices.push(index);
        return Ok(Some((base, indices)));
    }
    let base = resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::Base)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select base is missing".into()))?;
    if is_array_semantic(snapshot, base) {
        return Ok(Some((base, vec![index])));
    }
    let multidimensional_packed = base_semantic
        .type_id
        .map(|type_id| type_projector.project(type_id))
        .transpose()?
        .is_some_and(|projection| projection.packed_dimensions.len() > 1);
    Ok(multidimensional_packed.then(|| (raw_base, vec![index])))
}

type SemanticMemberPath = (Vec<String>, Vec<Option<NodeId>>);

fn member_path_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
    depth: usize,
) -> Result<Option<SemanticMemberPath>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "member access chain contains a cycle".into(),
        ));
    }
    let edges = semantic_edges(snapshot, node)?;
    let Some(base) = edge_target(ids, edges, SemanticEdgeRole::Base)? else {
        return Ok(None);
    };
    let base_semantic = &snapshot.semantic_nodes[base.index()];
    let (mut parts, mut refs) =
        if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 75 {
            let Some(path) =
                member_path_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
            else {
                return Ok(None);
            };
            path
        } else if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 65 {
            let Some(target) = expression_reference_target(snapshot, ids, base_semantic)? else {
                return Ok(None);
            };
            let target_semantic = &snapshot.semantic_nodes[target.index()];
            (vec![target_semantic.name.clone()], vec![Some(target)])
        } else if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 73 {
            // A member access on an unpacked virtual-interface array is captured
            // as `ArraySelect` followed by `MemberAccess`. Keep the element-select
            // node as the base reference so lowering can retrieve the runtime
            // handle from the container instead of collapsing it to the array
            // declaration.
            let Some((array, _indices)) =
                array_select_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
            else {
                return Ok(None);
            };
            let Some(name) = snapshot
                .semantic_nodes
                .get(array.index())
                .map(|array| array.name.clone())
                .filter(|name| !name.is_empty())
            else {
                return Ok(None);
            };
            (vec![name], vec![Some(semantic_id(ids, base_semantic.id)?)])
        } else {
            return Ok(None);
        };
    let member = node
        .target_id
        .map(|id| canonical_reference_target(snapshot, ids, id))
        .transpose()?;
    let member_name = member
        .and_then(|id| snapshot.semantic_nodes.get(id.index()))
        .map(|member| member.name.clone())
        .filter(|name| !name.is_empty())
        .or_else(|| (!node.name.is_empty()).then(|| node.name.clone()));
    let Some(member_name) = member_name else {
        return Ok(None);
    };
    parts.push(member_name);
    refs.push(member);
    Ok(Some((parts, refs)))
}

fn expression_reference_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
) -> Result<Option<NodeId>, DbError> {
    if let Some(target) = node.target_id {
        return canonical_reference_target(snapshot, ids, target).map(Some);
    }
    let edges = semantic_edges(snapshot, node)?;
    edge_target(ids, edges, SemanticEdgeRole::Reference)
}

fn virtual_interface_instance_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    variable: NodeId,
) -> Result<Option<NodeId>, DbError> {
    let variable_node = snapshot
        .semantic_nodes
        .get(variable.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface variable is missing".into()))?;
    if variable_node.kind != SemanticKind::Variable {
        return Ok(None);
    }
    let initializer = semantic_edges(snapshot, variable_node)?
        .iter()
        .find(|edge| edge.role == SemanticEdgeRole::Initializer)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()?;
    let Some(initializer) = initializer else {
        return Ok(None);
    };
    let Some(target) = expression_reference_target(
        snapshot,
        ids,
        snapshot
            .semantic_nodes
            .get(initializer.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface initializer is missing".into())
            })?,
    )?
    else {
        return Ok(None);
    };
    let target_node = snapshot
        .semantic_nodes
        .get(target.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface target is missing".into()))?;
    if target_node.kind != SemanticKind::Instance {
        return Ok(None);
    }
    let Some(definition) = target_node.target_id else {
        return Ok(None);
    };
    let definition = snapshot
        .semantic_nodes
        .get(definition as usize)
        .ok_or_else(|| {
            DbError::InvalidSnapshot("virtual interface definition is missing".into())
        })?;
    if definition.kind == SemanticKind::Definition
        && definition.definition_kind == Some(crate::ffi::slang::SemanticDefinitionKind::Interface)
    {
        return Ok(Some(target));
    }
    Ok(None)
}

fn find_clocking_member_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    owner: NodeId,
    name: &str,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "virtual interface member path contains a cycle".into(),
        ));
    }
    let owner_node = snapshot
        .semantic_nodes
        .get(owner.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface owner is missing".into()))?;
    for edge in semantic_edges(snapshot, owner_node)?
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Child)
    {
        let child = semantic_id(ids, edge.target_id)?;
        let child_node = snapshot.semantic_nodes.get(child.index()).ok_or_else(|| {
            DbError::InvalidSnapshot("virtual interface member is missing".into())
        })?;
        if child_node.name == name
            && ((child_node.kind == SemanticKind::Scope
                && child_node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK)
                || (child_node.kind == SemanticKind::Variable
                    && child_node.subkind == SEMANTIC_VARIABLE_CLOCKING))
        {
            return Ok(Some(child));
        }
        if matches!(
            child_node.kind,
            SemanticKind::Instance | SemanticKind::Scope
        ) {
            if let Some(found) =
                find_clocking_member_from_slang(snapshot, ids, child, name, depth + 1)?
            {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

fn clocking_block_from_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "clocking expression contains a cycle".into(),
        ));
    }
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking expression is missing".into()))?;
    if node.kind == SemanticKind::Scope && node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK {
        return Ok(Some(expression));
    }
    if node.kind != SemanticKind::Expression {
        return Ok(None);
    }
    if let Some(target) = expression_reference_target(snapshot, ids, node)? {
        let target_node = snapshot
            .semantic_nodes
            .get(target.index())
            .ok_or_else(|| DbError::InvalidSnapshot("clocking target is missing".into()))?;
        if target_node.kind == SemanticKind::Scope
            && target_node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
        {
            return Ok(Some(target));
        }
    }
    if node.subkind == 75 {
        let base = edge_target(ids, semantic_edges(snapshot, node)?, SemanticEdgeRole::Base)?;
        if let Some(base) = base {
            let base_node = snapshot.semantic_nodes.get(base.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface base is missing".into())
            })?;
            if let Some(variable) = expression_reference_target(snapshot, ids, base_node)? {
                if let Some(interface) =
                    virtual_interface_instance_from_slang(snapshot, ids, variable)?
                {
                    return find_clocking_member_from_slang(
                        snapshot,
                        ids,
                        interface,
                        &node.name,
                        depth + 1,
                    );
                }
            }
        }
    }
    let edges = semantic_edges(snapshot, node)?;
    let role = match node.subkind {
        72 => SemanticEdgeRole::Operand,
        73..=75 => SemanticEdgeRole::Base,
        _ => return Ok(None),
    };
    edge_target(ids, edges, role)?
        .map(|base| clocking_block_from_expression(snapshot, ids, base, depth + 1))
        .transpose()
        .map(|value| value.flatten())
}

fn clocking_source_from_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "clocking source expression contains a cycle".into(),
        ));
    }
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking source expression is missing".into()))?;
    if let Some(target) = expression_reference_target(snapshot, ids, node)? {
        let target_node = snapshot
            .semantic_nodes
            .get(target.index())
            .ok_or_else(|| DbError::InvalidSnapshot("clocking source target is missing".into()))?;
        if matches!(
            target_node.kind,
            SemanticKind::Net
                | SemanticKind::Variable
                | SemanticKind::Port
                | SemanticKind::Array
                | SemanticKind::NamedEvent
        ) {
            return Ok(Some(target));
        }
    }
    if node.kind != SemanticKind::Expression {
        return Ok(None);
    }
    if node.subkind == 75 {
        let base = edge_target(ids, semantic_edges(snapshot, node)?, SemanticEdgeRole::Base)?;
        if let Some(base) = base {
            let base_node = snapshot.semantic_nodes.get(base.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface base is missing".into())
            })?;
            if let Some(variable) = expression_reference_target(snapshot, ids, base_node)? {
                if let Some(interface) =
                    virtual_interface_instance_from_slang(snapshot, ids, variable)?
                {
                    return find_clocking_member_from_slang(
                        snapshot,
                        ids,
                        interface,
                        &node.name,
                        depth + 1,
                    );
                }
            }
        }
    }
    let edges = semantic_edges(snapshot, node)?;
    let role = match node.subkind {
        72 => SemanticEdgeRole::Operand,
        73..=75 => SemanticEdgeRole::Base,
        _ => return Ok(None),
    };
    edge_target(ids, edges, role)?
        .map(|base| clocking_source_from_expression(snapshot, ids, base, depth + 1))
        .transpose()
        .map(|value| value.flatten())
}

fn clocking_edge(code: u64) -> Result<ClockingEdge, DbError> {
    match code & CLOCKING_EDGE_MASK {
        0 => Ok(ClockingEdge::None),
        1 => Ok(ClockingEdge::Posedge),
        2 => Ok(ClockingEdge::Negedge),
        3 => Ok(ClockingEdge::BothEdges),
        _ => unreachable!("clocking edge mask has four values"),
    }
}

fn clocking_delay_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    delay: Option<NodeId>,
) -> Result<Option<NodeId>, DbError> {
    let Some(delay) = delay else {
        return Ok(None);
    };
    let timing = snapshot
        .semantic_nodes
        .get(delay.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking skew timing is missing".into()))?;
    if timing.subkind == 112 {
        return edge_target_at(
            ids,
            semantic_edges(snapshot, timing)?,
            SemanticEdgeRole::Delay,
            0,
        );
    }
    if timing.subkind == SEMANTIC_TIMING_ONE_STEP_DELAY {
        return Ok(None);
    }
    Err(DbError::InvalidSnapshot(format!(
        "unsupported clocking skew timing subkind {}",
        timing.subkind
    )))
}

fn clocking_skew_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    delay: Option<NodeId>,
    edge: u64,
) -> Result<ClockingSkew, DbError> {
    Ok(ClockingSkew {
        edge: clocking_edge(edge)?,
        delay,
        delay_expression: clocking_delay_expression(snapshot, ids, delay)?,
    })
}

fn peel_gate_terminal(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    mut expression: NodeId,
    direction: Direction,
) -> Result<NodeId, DbError> {
    let mut visited = HashSet::new();
    while visited.insert(expression) {
        let semantic = snapshot
            .semantic_nodes
            .get(expression.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("primitive terminal expression is missing".into())
            })?;
        if semantic.kind != SemanticKind::Expression {
            return Ok(expression);
        }
        let edges = semantic_edges(snapshot, semantic)?;
        if semantic.subkind == 72 && semantic.is_implicit_conversion {
            expression = edge_target(ids, edges, SemanticEdgeRole::Operand)?.ok_or_else(|| {
                DbError::InvalidSnapshot(
                    "implicit primitive terminal conversion has no operand".into(),
                )
            })?;
        } else if semantic.subkind == 71
            && matches!(direction, Direction::Output | Direction::Inout)
        {
            expression = edge_target(ids, edges, SemanticEdgeRole::Lhs)?.ok_or_else(|| {
                DbError::InvalidSnapshot("primitive output terminal has no lvalue".into())
            })?;
        } else {
            return Ok(expression);
        }
    }
    Err(DbError::InvalidSnapshot(
        "primitive terminal conversions contain a cycle".into(),
    ))
}

fn connection_source_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    mut expression: NodeId,
    direction: Direction,
) -> Result<NodeId, DbError> {
    let mut visited = HashSet::new();
    while visited.insert(expression) {
        let semantic = snapshot
            .semantic_nodes
            .get(expression.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("port connection expression is missing".into())
            })?;
        if semantic.kind != SemanticKind::Expression {
            return Ok(expression);
        }
        let edges = semantic_edges(snapshot, semantic)?;
        if semantic.subkind == 71 && matches!(direction, Direction::Output | Direction::Inout) {
            expression = edge_target(ids, edges, SemanticEdgeRole::Lhs)?.ok_or_else(|| {
                DbError::InvalidSnapshot("output port assignment wrapper has no lhs".into())
            })?;
            continue;
        }
        if semantic.subkind == 72 && semantic.is_implicit_conversion {
            expression = edge_target(ids, edges, SemanticEdgeRole::Operand)?.ok_or_else(|| {
                DbError::InvalidSnapshot("implicit port conversion has no operand".into())
            })?;
            continue;
        }
        return Ok(expression);
    }
    Err(DbError::InvalidSnapshot(
        "implicit port conversions contain a cycle".into(),
    ))
}

fn direction_from_slang(node: &SemanticNode) -> Direction {
    if node.is_input {
        Direction::Input
    } else if node.is_output {
        Direction::Output
    } else if node.is_inout {
        Direction::Inout
    } else if node.is_ref {
        Direction::Ref
    } else {
        Direction::None
    }
}

fn driver_delay(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
) -> Result<Option<DriverDelay>, DbError> {
    let Some(delay) = edge_target(ids, edges, SemanticEdgeRole::Delay)? else {
        return Ok(None);
    };
    let semantic = snapshot
        .semantic_nodes
        .get(delay.index())
        .ok_or_else(|| DbError::InvalidSnapshot("delay semantic node is missing".into()))?;
    if semantic.kind != SemanticKind::TimingControl {
        return Ok(Some(DriverDelay::Single(delay)));
    }
    let expressions = edge_targets(
        ids,
        semantic_edges(snapshot, semantic)?,
        SemanticEdgeRole::Delay,
    )?;
    Ok(Some(match expressions.as_slice() {
        [delay] => DriverDelay::Single(*delay),
        [rise, fall] => DriverDelay::RiseFall(*rise, *fall),
        [rise, fall, turn_off] => DriverDelay::RiseFallTurnOff(*rise, *fall, *turn_off),
        _ => {
            let location = source_position(snapshot, semantic)
                .ok()
                .and_then(|(file, line, col, _, _)| {
                    file.map(|file| format!(" at {file}:{line}:{col}"))
                })
                .unwrap_or_default();
            return Err(DbError::InvalidSnapshot(format!(
                "driver timing control{location} requires one to three delay expressions"
            )));
        }
    }))
}

fn value_data_from_slang(value: &SlangConstantValue) -> ValueData {
    match value {
        SlangConstantValue::None => ValueData::None,
        SlangConstantValue::Integer {
            is_signed,
            bit_width,
            value_words,
            unknown_words,
        } => ValueData::Vector {
            bit_width: *bit_width,
            is_signed: *is_signed,
            value_words: value_words.clone(),
            unknown_words: unknown_words.clone(),
        },
        SlangConstantValue::Real(value) => ValueData::Real(*value),
        SlangConstantValue::ShortReal(value) => ValueData::Real(f64::from(*value)),
        SlangConstantValue::String(value) => ValueData::Bytes(value.clone()),
        SlangConstantValue::Other(_) => ValueData::None,
    }
}

fn val_from_slang(value: &SlangConstantValue) -> Option<Val> {
    match value {
        SlangConstantValue::Integer {
            is_signed,
            bit_width,
            value_words,
            unknown_words,
        } => {
            let mut bits = Vec::with_capacity(usize::try_from(*bit_width).ok()?);
            for index in (0..*bit_width).rev() {
                let word = usize::try_from(index / 64).ok()?;
                let mask = 1_u64 << (index % 64);
                let value = value_words.get(word).is_some_and(|word| word & mask != 0);
                let unknown = unknown_words.get(word).is_some_and(|word| word & mask != 0);
                bits.push(match (unknown, value) {
                    (false, false) => crate::core::elab::Bit::Zero,
                    (false, true) => crate::core::elab::Bit::One,
                    (true, false) => crate::core::elab::Bit::X,
                    (true, true) => crate::core::elab::Bit::Z,
                });
            }
            Some(Val::Bits(crate::core::elab::Value::from_bits(
                bits, *is_signed,
            )))
        }
        SlangConstantValue::Real(value) => Some(Val::Real(*value)),
        SlangConstantValue::ShortReal(value) => Some(Val::Real(f64::from(*value))),
        SlangConstantValue::String(value) => String::from_utf8(value.clone()).ok().map(Val::Str),
        SlangConstantValue::None | SlangConstantValue::Other(_) => None,
    }
}

fn operation_from_slang(operation: SemanticOperation, unary: bool) -> Operation {
    match operation {
        SemanticOperation::None => Operation::Null,
        SemanticOperation::Plus if unary => Operation::UnaryPlus,
        SemanticOperation::Minus if unary => Operation::UnaryMinus,
        SemanticOperation::Plus => Operation::Add,
        SemanticOperation::Minus => Operation::Subtract,
        SemanticOperation::Multiply => Operation::Multiply,
        SemanticOperation::Divide => Operation::Divide,
        SemanticOperation::Modulo => Operation::Modulo,
        SemanticOperation::Power => Operation::Power,
        SemanticOperation::BitNot => Operation::BitwiseNot,
        SemanticOperation::BitAnd if unary => Operation::ReductionAnd,
        SemanticOperation::BitOr if unary => Operation::ReductionOr,
        SemanticOperation::BitXor if unary => Operation::ReductionXor,
        SemanticOperation::BitNand if unary => Operation::ReductionNand,
        SemanticOperation::BitNor if unary => Operation::ReductionNor,
        SemanticOperation::BitXnor if unary => Operation::ReductionXnor,
        SemanticOperation::BitAnd | SemanticOperation::BitNand => Operation::BitwiseAnd,
        SemanticOperation::BitOr | SemanticOperation::BitNor => Operation::BitwiseOr,
        SemanticOperation::BitXor => Operation::BitwiseXor,
        SemanticOperation::BitXnor => Operation::BitwiseXnor,
        SemanticOperation::LogicalNot => Operation::LogicalNot,
        SemanticOperation::LogicalAnd => Operation::LogicalAnd,
        SemanticOperation::LogicalOr => Operation::LogicalOr,
        SemanticOperation::LogicalImplication => Operation::Imply,
        SemanticOperation::LogicalEquivalence => Operation::LogicalEquivalence,
        SemanticOperation::Equal => Operation::Equal,
        SemanticOperation::NotEqual => Operation::NotEqual,
        SemanticOperation::CaseEqual => Operation::CaseEqual,
        SemanticOperation::CaseNotEqual => Operation::CaseNotEqual,
        SemanticOperation::WildcardEqual => Operation::WildEqual,
        SemanticOperation::WildcardNotEqual => Operation::WildNotEqual,
        SemanticOperation::Greater => Operation::Greater,
        SemanticOperation::GreaterEqual => Operation::GreaterEqual,
        SemanticOperation::Less => Operation::Less,
        SemanticOperation::LessEqual => Operation::LessEqual,
        SemanticOperation::ShiftLeft => Operation::ShiftLeft,
        SemanticOperation::ShiftRight => Operation::ShiftRight,
        SemanticOperation::ArithmeticShiftLeft => Operation::ArithmeticShiftLeft,
        SemanticOperation::ArithmeticShiftRight => Operation::ArithmeticShiftRight,
        SemanticOperation::PreIncrement => Operation::PreIncrement,
        SemanticOperation::PreDecrement => Operation::PreDecrement,
        SemanticOperation::PostIncrement => Operation::PostIncrement,
        SemanticOperation::PostDecrement => Operation::PostDecrement,
        SemanticOperation::Concat => Operation::Concat,
        SemanticOperation::Replicate => Operation::MultiConcat,
        SemanticOperation::Conditional => Operation::Conditional,
        SemanticOperation::StreamLeft => Operation::StreamLeftToRight,
        SemanticOperation::StreamRight => Operation::StreamRightToLeft,
        SemanticOperation::Assign => Operation::Assignment,
        SemanticOperation::Inside => Operation::Inside,
        SemanticOperation::AssignmentPattern => Operation::AssignmentPattern,
        SemanticOperation::MinTypMax => Operation::MinTypMax,
        SemanticOperation::MultiAssignmentPattern => Operation::MultiAssignmentPattern,
        SemanticOperation::List => Operation::List,
        SemanticOperation::AssertionAnd
        | SemanticOperation::AssertionOr
        | SemanticOperation::AssertionIntersect
        | SemanticOperation::AssertionThroughout
        | SemanticOperation::AssertionWithin
        | SemanticOperation::AssertionIff
        | SemanticOperation::AssertionUntil
        | SemanticOperation::AssertionSUntil
        | SemanticOperation::AssertionUntilWith
        | SemanticOperation::AssertionSUntilWith
        | SemanticOperation::AssertionImplies
        | SemanticOperation::AssertionOverlappedImplies
        | SemanticOperation::AssertionNonOverlappedImplies
        | SemanticOperation::AssertionOverlappedFollowedBy
        | SemanticOperation::AssertionNonOverlappedFollowedBy
        | SemanticOperation::AssertionNot
        | SemanticOperation::AssertionNextTime
        | SemanticOperation::AssertionSNextTime
        | SemanticOperation::AssertionAlways
        | SemanticOperation::AssertionSAlways
        | SemanticOperation::AssertionEventually
        | SemanticOperation::AssertionSEventually => Operation::Null,
    }
}

fn time_exponent(scale: Option<SemanticTimeScale>, precision: bool) -> Result<i32, DbError> {
    let Some(scale) = scale else {
        return Ok(if precision { -12 } else { -9 });
    };
    let (unit, magnitude) = if precision {
        (scale.precision_unit, scale.precision_magnitude)
    } else {
        (scale.unit, scale.magnitude)
    };
    let base = match unit {
        SemanticTimeUnit::Seconds => 0,
        SemanticTimeUnit::Milliseconds => -3,
        SemanticTimeUnit::Microseconds => -6,
        SemanticTimeUnit::Nanoseconds => -9,
        SemanticTimeUnit::Picoseconds => -12,
        SemanticTimeUnit::Femtoseconds => -15,
    };
    let offset = match magnitude {
        1 => 0,
        10 => 1,
        100 => 2,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "invalid Slang time scale magnitude".into(),
            ))
        }
    };
    Ok(base + offset)
}

fn time_literal_scale(scale: Option<SemanticTimeScale>) -> Option<TimeLiteralScale> {
    scale.map(|scale| TimeLiteralScale {
        unit: match scale.unit {
            SemanticTimeUnit::Seconds => TimeUnit::Seconds,
            SemanticTimeUnit::Milliseconds => TimeUnit::Milliseconds,
            SemanticTimeUnit::Microseconds => TimeUnit::Microseconds,
            SemanticTimeUnit::Nanoseconds => TimeUnit::Nanoseconds,
            SemanticTimeUnit::Picoseconds => TimeUnit::Picoseconds,
            SemanticTimeUnit::Femtoseconds => TimeUnit::Femtoseconds,
        },
        magnitude: scale.magnitude,
    })
}

fn net_type_from_subkind(subkind: u32) -> NetType {
    match subkind {
        128 => NetType::Wire,
        129 => NetType::Wand,
        130 => NetType::Wor,
        131 => NetType::Tri,
        132 => NetType::TriAnd,
        133 => NetType::TriOr,
        134 => NetType::Tri0,
        135 => NetType::Tri1,
        136 => NetType::TriReg,
        137 => NetType::Supply0,
        138 => NetType::Supply1,
        139 => NetType::Uwire,
        140 | 141 => NetType::Unsupported,
        _ => NetType::Unsupported,
    }
}

fn primitive_type_from_subkind(subkind: u32) -> PrimitiveType {
    match subkind {
        200 => PrimitiveType::And,
        201 => PrimitiveType::Nand,
        202 => PrimitiveType::Nor,
        203 => PrimitiveType::Or,
        204 => PrimitiveType::Xor,
        205 => PrimitiveType::Xnor,
        206 => PrimitiveType::Buf,
        207 => PrimitiveType::Not,
        208 => PrimitiveType::Bufif0,
        209 => PrimitiveType::Bufif1,
        210 => PrimitiveType::Notif0,
        211 => PrimitiveType::Notif1,
        212 => PrimitiveType::Nmos,
        213 => PrimitiveType::Pmos,
        214 => PrimitiveType::Cmos,
        215 => PrimitiveType::Rnmos,
        216 => PrimitiveType::Rpmos,
        217 => PrimitiveType::Rcmos,
        218 => PrimitiveType::Rtran,
        219 => PrimitiveType::Rtranif0,
        220 => PrimitiveType::Rtranif1,
        221 => PrimitiveType::Tran,
        222 => PrimitiveType::Tranif0,
        223 => PrimitiveType::Tranif1,
        224 => PrimitiveType::Pullup,
        225 => PrimitiveType::Pulldown,
        226 => PrimitiveType::Sequential,
        227 => PrimitiveType::Combinational,
        _ => PrimitiveType::Unsupported,
    }
}

fn strength_from_slang(strength: SemanticDriveStrength) -> Strength {
    match strength {
        SemanticDriveStrength::Unspecified => Strength::Unspecified,
        SemanticDriveStrength::Supply => Strength::Supply,
        SemanticDriveStrength::Strong => Strength::Strong,
        SemanticDriveStrength::Pull => Strength::Pull,
        SemanticDriveStrength::Weak => Strength::Weak,
        SemanticDriveStrength::HighZ => Strength::HighZ,
    }
}

fn assertion_unary_from_slang(operation: SemanticOperation) -> Result<AssertionUnaryOp, DbError> {
    Ok(match operation {
        SemanticOperation::AssertionNot => AssertionUnaryOp::Not,
        SemanticOperation::AssertionNextTime => AssertionUnaryOp::NextTime,
        SemanticOperation::AssertionSNextTime => AssertionUnaryOp::SNextTime,
        SemanticOperation::AssertionAlways => AssertionUnaryOp::Always,
        SemanticOperation::AssertionSAlways => AssertionUnaryOp::SAlways,
        SemanticOperation::AssertionEventually => AssertionUnaryOp::Eventually,
        SemanticOperation::AssertionSEventually => AssertionUnaryOp::SEventually,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion unary node has a non-unary operation".into(),
            ))
        }
    })
}

fn assertion_binary_from_slang(operation: SemanticOperation) -> Result<AssertionBinaryOp, DbError> {
    Ok(match operation {
        SemanticOperation::AssertionAnd => AssertionBinaryOp::And,
        SemanticOperation::AssertionOr => AssertionBinaryOp::Or,
        SemanticOperation::AssertionIntersect => AssertionBinaryOp::Intersect,
        SemanticOperation::AssertionThroughout => AssertionBinaryOp::Throughout,
        SemanticOperation::AssertionWithin => AssertionBinaryOp::Within,
        SemanticOperation::AssertionIff => AssertionBinaryOp::Iff,
        SemanticOperation::AssertionUntil => AssertionBinaryOp::Until,
        SemanticOperation::AssertionSUntil => AssertionBinaryOp::SUntil,
        SemanticOperation::AssertionUntilWith => AssertionBinaryOp::UntilWith,
        SemanticOperation::AssertionSUntilWith => AssertionBinaryOp::SUntilWith,
        SemanticOperation::AssertionImplies => AssertionBinaryOp::Implies,
        SemanticOperation::AssertionOverlappedImplies => AssertionBinaryOp::OverlappedImplication,
        SemanticOperation::AssertionNonOverlappedImplies => {
            AssertionBinaryOp::NonOverlappedImplication
        }
        SemanticOperation::AssertionOverlappedFollowedBy => AssertionBinaryOp::OverlappedFollowedBy,
        SemanticOperation::AssertionNonOverlappedFollowedBy => {
            AssertionBinaryOp::NonOverlappedFollowedBy
        }
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion binary node has a non-binary operation".into(),
            ))
        }
    })
}

fn assertion_expr_from_slang(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    let kind = match node.subkind {
        1 => AssertionExprKind::Invalid {
            child: first(SemanticEdgeRole::Body)?,
        },
        2 => AssertionExprKind::Simple {
            expr: required(SemanticEdgeRole::Operand, "simple assertion operand")?,
            repeated: node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0,
        },
        3 => AssertionExprKind::SequenceConcat {
            elements: edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
        },
        4 => AssertionExprKind::SequenceWithMatch {
            expr: required(SemanticEdgeRole::Body, "sequence match body")?,
            match_items: edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
            repeated: node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0,
        },
        5 => AssertionExprKind::Unary {
            op: assertion_unary_from_slang(node.operation)?,
            expr: required(SemanticEdgeRole::Body, "unary assertion body")?,
            ranged: node.auxiliary & SEMANTIC_ASSERTION_RANGE != 0,
        },
        6 => AssertionExprKind::Binary {
            op: assertion_binary_from_slang(node.operation)?,
            left: required(SemanticEdgeRole::Left, "assertion binary left")?,
            right: required(SemanticEdgeRole::Right, "assertion binary right")?,
        },
        7 => AssertionExprKind::FirstMatch {
            sequence: required(SemanticEdgeRole::Body, "first_match sequence")?,
            match_items: edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
        },
        8 => {
            let control = required(SemanticEdgeRole::Clocking, "assertion clocking")?;
            let timing = snapshot
                .semantic_nodes
                .get(control.index())
                .ok_or_else(|| DbError::InvalidSnapshot("assertion clocking is missing".into()))?;
            let timing_edges = semantic_edges(snapshot, timing)?;
            let signal = edge_target(ids, timing_edges, SemanticEdgeRole::Event)?
                .ok_or_else(|| DbError::InvalidSnapshot("assertion clock has no signal".into()))?;
            AssertionExprKind::Clocking {
                control,
                signal,
                posedge: timing.is_posedge,
                expr: required(SemanticEdgeRole::Body, "clocked assertion body")?,
            }
        }
        9 => AssertionExprKind::StrongWeak {
            expr: required(SemanticEdgeRole::Body, "strong/weak assertion body")?,
            strong: node.auxiliary & SEMANTIC_ASSERTION_STRONG != 0,
        },
        10 => AssertionExprKind::Abort {
            condition: required(SemanticEdgeRole::Condition, "abort condition")?,
            expr: required(SemanticEdgeRole::Body, "abort assertion body")?,
            reject: node.auxiliary & SEMANTIC_ASSERTION_ABORT_REJECT != 0,
            sync: node.auxiliary & SEMANTIC_ASSERTION_ABORT_SYNC != 0,
        },
        11 => AssertionExprKind::Conditional {
            condition: required(SemanticEdgeRole::Condition, "assertion conditional")?,
            if_expr: required(SemanticEdgeRole::Then, "assertion conditional then")?,
            else_expr: first(SemanticEdgeRole::Else)?,
        },
        12 => {
            let expr = required(
                SemanticEdgeRole::CaseExpression,
                "assertion case expression",
            )?;
            let branch_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Branch)
                .map(|edge| edge.index)
                .max()
                .map_or(0, |index| index.saturating_add(1));
            let mut items = Vec::with_capacity(branch_count as usize);
            for item_index in 0..branch_count {
                let expressions = edges
                    .iter()
                    .filter(|edge| {
                        edge.role == SemanticEdgeRole::CaseItem && edge.index >> 16 == item_index
                    })
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?;
                let body = edges
                    .iter()
                    .find(|edge| edge.role == SemanticEdgeRole::Branch && edge.index == item_index)
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .transpose()?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("assertion case body is missing".into())
                    })?;
                items.push(AssertionCaseItem { expressions, body });
            }
            AssertionExprKind::Case {
                expr,
                items,
                default_case: first(SemanticEdgeRole::Else)?,
            }
        }
        13 => AssertionExprKind::DisableIff {
            condition: required(SemanticEdgeRole::Condition, "disable iff condition")?,
            expr: required(SemanticEdgeRole::Body, "disable iff assertion body")?,
        },
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion expression has an unknown subkind".into(),
            ))
        }
    };
    Ok(NodeKind::AssertionExpr(kind))
}

fn node_kind_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
    ty: TypeInfo,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    Ok(match node.kind {
        SemanticKind::Instance if node.subkind == 193 => NodeKind::InstanceArray,
        SemanticKind::Instance | SemanticKind::Definition => NodeKind::ModuleInst {
            def_name: if node.definition_name.is_empty() {
                node.name.clone()
            } else {
                node.definition_name.clone()
            },
            is_top: node.is_top,
            is_interface: node.definition_kind == Some(SemanticDefinitionKind::Interface),
            timeunit: time_exponent(node.time_scale, false)?,
            timeprecision: time_exponent(node.time_scale, true)?,
        },
        SemanticKind::Package => NodeKind::Package,
        SemanticKind::Class => NodeKind::ClassDef,
        SemanticKind::GenerateScope if node.subkind == 196 => NodeKind::GenScopeArray,
        SemanticKind::GenerateScope => NodeKind::GenScope,
        SemanticKind::Port => {
            let direction = direction_from_slang(node);
            let high_expr = first(SemanticEdgeRole::HighConnection)?
                .map(|expression| {
                    connection_source_expression(snapshot, ids, expression, direction)
                })
                .transpose()?;
            let high =
                resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::HighConnection)?;
            NodeKind::Port {
                direction,
                ty,
                strength0: strength_from_slang(node.strength0),
                strength1: strength_from_slang(node.strength1),
                high,
                low: resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::LowConnection)?,
                high_expr,
                high_present: node.port_connection_present,
                high_open: node.port_connection_open,
            }
        }
        SemanticKind::Modport => NodeKind::ModPort,
        SemanticKind::InterfaceConnection => NodeKind::IfaceConn {
            actual: node
                .target_id
                .map(|id| semantic_id(ids, id))
                .transpose()?
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("interface connection has no target".into())
                })?,
            modport: node.name.clone(),
        },
        SemanticKind::Net => NodeKind::Net {
            ty,
            net_type: net_type_from_subkind(node.subkind),
            strength0: strength_from_slang(node.strength0),
            strength1: strength_from_slang(node.strength1),
        },
        SemanticKind::NetAlias => NodeKind::NetAlias {
            nets: edge_targets(ids, edges, SemanticEdgeRole::AliasNet)?,
        },
        SemanticKind::Variable if node.subkind == 229 => NodeKind::Genvar { ty },
        SemanticKind::Variable => NodeKind::Var { ty },
        SemanticKind::Array => NodeKind::Array { ty },
        SemanticKind::NamedEvent => NodeKind::NamedEvent,
        SemanticKind::Parameter => NodeKind::Param {
            ty,
            value: node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
            local: node.is_local,
        },
        SemanticKind::Process => NodeKind::Process {
            kind: match node.subkind {
                1 => ProcessKind::Initial,
                2 => ProcessKind::Final,
                4 => ProcessKind::Always {
                    always_type: AlwaysKind::Comb,
                },
                5 => ProcessKind::Always {
                    always_type: AlwaysKind::Latch,
                },
                6 => ProcessKind::Always {
                    always_type: AlwaysKind::FlipFlop,
                },
                _ => ProcessKind::Always {
                    always_type: AlwaysKind::Always,
                },
            },
        },
        SemanticKind::ContinuousAssign => NodeKind::ContAssign {
            net_decl: node.subkind == 228,
            delay: driver_delay(snapshot, ids, edges)?,
            strength0: strength_from_slang(node.strength0),
            strength1: strength_from_slang(node.strength1),
        },
        SemanticKind::Primitive if node.is_primitive_instance => {
            let terms = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Actual)
                .map(|edge| {
                    let expression = snapshot
                        .semantic_nodes
                        .get(edge.target_id as usize)
                        .ok_or_else(|| {
                            DbError::InvalidSnapshot(
                                "primitive terminal expression is missing".into(),
                            )
                        })?;
                    let direction = direction_from_slang(expression);
                    Ok(GateTerm {
                        direction,
                        term_index: i32::try_from(edge.index).map_err(|_| {
                            DbError::InvalidSnapshot("primitive terminal index is too large".into())
                        })?,
                        expr: peel_gate_terminal(
                            snapshot,
                            ids,
                            semantic_id(ids, edge.target_id)?,
                            direction,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            let prim_type = primitive_type_from_subkind(node.subkind);
            let is_array_element = node
                .parent_id
                .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
                .is_some_and(|parent| {
                    parent.kind == SemanticKind::Instance && parent.subkind == 193
                });
            NodeKind::Gate {
                class: if is_array_element {
                    PrimClass::Array
                } else if matches!(
                    prim_type,
                    PrimitiveType::Sequential | PrimitiveType::Combinational
                ) {
                    PrimClass::Udp
                } else if matches!(
                    prim_type,
                    PrimitiveType::Nmos
                        | PrimitiveType::Pmos
                        | PrimitiveType::Cmos
                        | PrimitiveType::Rnmos
                        | PrimitiveType::Rpmos
                        | PrimitiveType::Rcmos
                        | PrimitiveType::Rtran
                        | PrimitiveType::Rtranif0
                        | PrimitiveType::Rtranif1
                        | PrimitiveType::Tran
                        | PrimitiveType::Tranif0
                        | PrimitiveType::Tranif1
                ) {
                    PrimClass::Switch
                } else {
                    PrimClass::Gate
                },
                prim_type,
                strength0: strength_from_slang(node.strength0),
                strength1: strength_from_slang(node.strength1),
                delay: driver_delay(snapshot, ids, edges)?,
                terms,
            }
        }
        SemanticKind::Primitive => NodeKind::Other,
        SemanticKind::Subroutine => NodeKind::FuncTask {
            is_task: node.is_task,
            automatic: node.is_automatic,
            is_static: node.auxiliary & crate::ffi::slang::SUBROUTINE_STATIC != 0,
            is_virtual: node.auxiliary & crate::ffi::slang::SUBROUTINE_VIRTUAL != 0,
            is_pure: node.auxiliary & crate::ffi::slang::SUBROUTINE_PURE != 0,
            is_final: node.auxiliary & crate::ffi::slang::SUBROUTINE_FINAL != 0,
            is_constructor: node.auxiliary & crate::ffi::slang::SUBROUTINE_CONSTRUCTOR != 0,
            ret: (!node.is_task && ty.kind != "void").then_some(ty),
            body: first(SemanticEdgeRole::Body)?,
        },
        SemanticKind::Argument => NodeKind::FuncArg {
            direction: direction_from_slang(node),
            ty,
            default: first(SemanticEdgeRole::DefaultValue)?,
            const_ref: node.is_const_ref,
            ref_static: node.is_ref_static,
        },
        SemanticKind::Statement => statement_from_slang(snapshot, node, edges, ids)?,
        SemanticKind::Expression => {
            expression_from_slang(snapshot, type_projector, node, edges, ids, ty)?
        }
        SemanticKind::AssertionExpr => assertion_expr_from_slang(snapshot, node, edges, ids)?,
        SemanticKind::SystemCall => NodeKind::SysCall {
            name: node.name.clone(),
        },
        SemanticKind::MethodCall => NodeKind::MethodCall {
            name: node.name.clone(),
            receiver: first(SemanticEdgeRole::Receiver)?,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::FunctionCall => NodeKind::FuncCall {
            name: node.name.clone(),
            is_task: node.is_task,
            is_super: node.auxiliary & crate::ffi::slang::CALL_SUPER != 0,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::EnumConstant => NodeKind::EnumConst {
            value: node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
        },
        SemanticKind::Scope => NodeKind::Stmt(StmtKind::Begin),
        SemanticKind::TimingControl => NodeKind::Other,
        SemanticKind::Unsupported => NodeKind::Other,
    })
}

fn statement_from_slang(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    Ok(NodeKind::Stmt(match node.subkind {
        32 | 60 => StmtKind::Begin,
        SEMANTIC_STMT_IMMEDIATE_ASSERT
        | SEMANTIC_STMT_IMMEDIATE_ASSUME
        | SEMANTIC_STMT_IMMEDIATE_COVER => {
            if node.auxiliary & !(SEMANTIC_ASSERTION_DEFERRED | SEMANTIC_ASSERTION_FINAL) != 0 {
                return Err(DbError::InvalidSnapshot(
                    "immediate assertion has unknown metadata".into(),
                ));
            }
            StmtKind::ImmediateAssertion {
                kind: match node.subkind {
                    SEMANTIC_STMT_IMMEDIATE_ASSERT => ImmediateAssertionKind::Assert,
                    SEMANTIC_STMT_IMMEDIATE_ASSUME => ImmediateAssertionKind::Assume,
                    SEMANTIC_STMT_IMMEDIATE_COVER => ImmediateAssertionKind::Cover,
                    _ => unreachable!("immediate assertion subkind was prevalidated"),
                },
                cond: required(SemanticEdgeRole::Condition, "assertion condition")?,
                if_true: first(SemanticEdgeRole::Then)?,
                if_false: first(SemanticEdgeRole::Else)?,
                label: node.name.clone(),
                deferred: node.auxiliary & SEMANTIC_ASSERTION_DEFERRED != 0,
                is_final: node.auxiliary & SEMANTIC_ASSERTION_FINAL != 0,
            }
        }
        SEMANTIC_STMT_CONCURRENT_ASSERT
        | SEMANTIC_STMT_CONCURRENT_ASSUME
        | SEMANTIC_STMT_CONCURRENT_COVER => StmtKind::ConcurrentAssertion {
            kind: match node.subkind {
                SEMANTIC_STMT_CONCURRENT_ASSERT => ConcurrentAssertionKind::Assert,
                SEMANTIC_STMT_CONCURRENT_ASSUME => ConcurrentAssertionKind::Assume,
                SEMANTIC_STMT_CONCURRENT_COVER => ConcurrentAssertionKind::Cover,
                _ => unreachable!("concurrent assertion subkind was prevalidated"),
            },
            property: required(SemanticEdgeRole::PropertySpec, "assertion property")?,
            if_true: first(SemanticEdgeRole::Then)?,
            if_false: first(SemanticEdgeRole::Else)?,
            label: node.name.clone(),
        },
        33 => StmtKind::IfElse {
            cond: required(SemanticEdgeRole::Condition, "if condition")?,
            check: unique_priority_check(node.auxiliary)?,
        },
        34 => {
            let mut items = Vec::new();
            let branch_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Branch)
                .map(|edge| edge.index)
                .max()
                .map_or(0, |index| index.saturating_add(1));
            for item_index in 0..branch_count {
                let exprs = edges
                    .iter()
                    .filter(|edge| {
                        edge.role == SemanticEdgeRole::CaseItem && edge.index >> 16 == item_index
                    })
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?;
                let body = edges
                    .iter()
                    .find(|edge| edge.role == SemanticEdgeRole::Branch && edge.index == item_index)
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .transpose()?;
                items.push(CaseItem { exprs, body });
            }
            if let Some(default) = first(SemanticEdgeRole::Else)? {
                items.push(CaseItem {
                    exprs: Vec::new(),
                    body: Some(default),
                });
            }
            StmtKind::Case {
                case_type: if node.case_inside {
                    CaseKind::Inside
                } else if node.case_wildcard_x_or_z {
                    CaseKind::X
                } else if node.case_wildcard_z {
                    CaseKind::Z
                } else {
                    CaseKind::Exact
                },
                check: unique_priority_check(node.auxiliary)?,
                items,
            }
        }
        35 => StmtKind::For {
            vars: Vec::new(),
            init: edge_targets(ids, edges, SemanticEdgeRole::Initializer)?,
            cond: required(SemanticEdgeRole::Condition, "for condition")?,
            incr: edge_targets(ids, edges, SemanticEdgeRole::Increment)?,
            body: required(SemanticEdgeRole::Body, "for body")?,
        },
        36 => StmtKind::While {
            cond: required(SemanticEdgeRole::Condition, "while condition")?,
            body: required(SemanticEdgeRole::Body, "while body")?,
        },
        37 => StmtKind::DoWhile {
            cond: required(SemanticEdgeRole::Condition, "do-while condition")?,
            body: required(SemanticEdgeRole::Body, "do-while body")?,
        },
        38 => StmtKind::Repeat {
            cond: required(SemanticEdgeRole::Condition, "repeat count")?,
            body: required(SemanticEdgeRole::Body, "repeat body")?,
        },
        39 => StmtKind::Forever {
            body: required(SemanticEdgeRole::Body, "forever body")?,
        },
        42 if node.auxiliary == 1 => {
            let mut events = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Event)
                .collect::<Vec<_>>();
            events.sort_by_key(|edge| edge.index);
            StmtKind::WaitOrder {
                events: events
                    .into_iter()
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?,
                if_true: first(SemanticEdgeRole::Then)?,
                if_false: first(SemanticEdgeRole::Else)?,
            }
        }
        42 => StmtKind::Wait {
            cond: required(SemanticEdgeRole::Condition, "wait condition")?,
        },
        43 => StmtKind::Return {
            value: first(SemanticEdgeRole::Body)?,
        },
        44 => StmtKind::Break,
        45 => StmtKind::Continue,
        46 => StmtKind::Disable {
            target: node
                .target_id
                .map(|symbol| block_statement_for_symbol(snapshot, ids, symbol))
                .transpose()?
                .flatten(),
        },
        47 => StmtKind::Empty,
        48 => {
            let expression = required(SemanticEdgeRole::Body, "expression statement body")?;
            let semantic = &snapshot.semantic_nodes[expression.index()];
            if semantic.subkind == 71 {
                let assignment_edges = semantic_edges(snapshot, semantic)?;
                if edge_target(ids, assignment_edges, SemanticEdgeRole::Lhs)?.is_none()
                    || edge_target(ids, assignment_edges, SemanticEdgeRole::Rhs)?.is_none()
                {
                    return Err(DbError::InvalidSnapshot(
                        "assignment expression is missing an operand".into(),
                    ));
                }
                StmtKind::Assign {
                    blocking: !semantic.is_nonblocking,
                    op: operation_from_slang(semantic.operation, false),
                    delay: intra_control(snapshot, semantic, assignment_edges, ids)?,
                }
            } else {
                StmtKind::Begin
            }
        }
        49 => StmtKind::VariableDecl {
            declaration: required(
                SemanticEdgeRole::Declaration,
                "variable declaration statement declaration",
            )?,
        },
        50 => StmtKind::ProcContAssign {
            lhs: required(SemanticEdgeRole::Lhs, "procedural assignment lhs")?,
            rhs: required(SemanticEdgeRole::Rhs, "procedural assignment rhs")?,
        },
        51 => StmtKind::Force {
            lhs: required(SemanticEdgeRole::Lhs, "force lhs")?,
            rhs: required(SemanticEdgeRole::Rhs, "force rhs")?,
        },
        52 => StmtKind::Deassign {
            lhs: required(SemanticEdgeRole::Lhs, "deassign lhs")?,
        },
        53 => StmtKind::Release {
            lhs: required(SemanticEdgeRole::Lhs, "release lhs")?,
        },
        54 => StmtKind::WaitFork,
        55 => StmtKind::DisableFork,
        56..=58 => {
            let body = required(SemanticEdgeRole::Body, "fork body")?;
            let body_semantic = snapshot.semantic_nodes.get(body.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("fork body semantic node is missing".into())
            })?;
            let branches =
                if body_semantic.kind == SemanticKind::Statement && body_semantic.subkind == 60 {
                    let body_edges = semantic_edges(snapshot, body_semantic)?;
                    let mut branches = edge_targets(ids, body_edges, SemanticEdgeRole::Child)?;
                    if branches.is_empty() {
                        branches = edge_targets(ids, body_edges, SemanticEdgeRole::Body)?;
                    }
                    branches
                } else {
                    vec![body]
                };
            StmtKind::Fork {
                target: node
                    .target_id
                    .map(|symbol| block_statement_for_symbol(snapshot, ids, symbol))
                    .transpose()?
                    .flatten(),
                join_kind: match node.subkind {
                    56 => JoinKind::All,
                    57 => JoinKind::Any,
                    _ => JoinKind::None,
                },
                branches,
            }
        }
        59 => {
            let encoded_count = usize::try_from(node.auxiliary).map_err(|_| {
                DbError::InvalidSnapshot("foreach dimension count is too large".into())
            })?;
            let edge_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
                .map(|edge| {
                    usize::try_from(edge.index).map_err(|_| {
                        DbError::InvalidSnapshot("foreach declaration index is too large".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .map_or(0, |index| index.saturating_add(1));
            let count = if encoded_count == 0 {
                edge_count
            } else {
                encoded_count
            };
            let mut vars = vec![None; count];
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
            {
                let index = usize::try_from(edge.index).map_err(|_| {
                    DbError::InvalidSnapshot("foreach declaration index is too large".into())
                })?;
                let slot = vars.get_mut(index).ok_or_else(|| {
                    DbError::InvalidSnapshot(
                        "foreach declaration index exceeds its dimension count".into(),
                    )
                })?;
                if slot.replace(semantic_id(ids, edge.target_id)?).is_some() {
                    return Err(DbError::InvalidSnapshot(
                        "foreach has duplicate declaration index".into(),
                    ));
                }
            }
            StmtKind::Foreach {
                array: resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::Base)?,
                vars,
                body: required(SemanticEdgeRole::Body, "foreach body")?,
            }
        }
        40 => timing_statement(snapshot, node, edges, ids)?,
        41 => StmtKind::EventTrigger {
            blocking: !node.is_nonblocking,
            target: edge_target(ids, edges, SemanticEdgeRole::Event)?,
            timing: edge_target(ids, edges, SemanticEdgeRole::Delay)?
                .map(|timing| event_trigger_timing(snapshot, timing, ids))
                .transpose()?,
        },
        _ => StmtKind::Unsupported {
            object_type: ObjectType::UnsupportedStatement,
        },
    }))
}

fn unique_priority_check(value: u64) -> Result<UniquePriorityCheck, DbError> {
    Ok(match value {
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_NONE => UniquePriorityCheck::None,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_UNIQUE => UniquePriorityCheck::Unique,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_UNIQUE0 => UniquePriorityCheck::Unique0,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_PRIORITY => UniquePriorityCheck::Priority,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "statement has an unknown unique/priority qualifier".into(),
            ))
        }
    })
}

fn block_statement_for_symbol(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    symbol: u64,
) -> Result<Option<NodeId>, DbError> {
    let mut statements = snapshot.semantic_nodes.iter().filter(|candidate| {
        candidate.kind == SemanticKind::Statement
            && matches!(candidate.subkind, 32 | 56..=58)
            && candidate.target_id == Some(symbol)
    });
    let result = statements
        .next()
        .map(|statement| semantic_id(ids, statement.id))
        .transpose()?;
    if statements.next().is_some() {
        return Err(DbError::InvalidSnapshot(
            "block symbol is owned by multiple statements".into(),
        ));
    }
    if result.is_some() {
        return Ok(result);
    }
    let symbol = semantic_id(ids, symbol)?;
    Ok(matches!(
        snapshot
            .semantic_nodes
            .get(symbol.index())
            .map(|node| node.kind),
        Some(SemanticKind::Subroutine)
    )
    .then_some(symbol))
}

fn intra_control(
    snapshot: &SlangSnapshot,
    _assignment: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<Option<IntraControl>, DbError> {
    let Some(timing_id) = edge_target(ids, edges, SemanticEdgeRole::Delay)? else {
        return Ok(None);
    };
    Ok(Some(intra_control_timing(snapshot, timing_id, ids)?))
}

fn intra_control_timing(
    snapshot: &SlangSnapshot,
    timing_id: NodeId,
    ids: &HashMap<u64, NodeId>,
) -> Result<IntraControl, DbError> {
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| {
            DbError::InvalidSnapshot("assignment timing control node is missing".into())
        })?;
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        112 => {
            let delay = edge_target(ids, edges, SemanticEdgeRole::Delay)?.ok_or_else(|| {
                DbError::InvalidSnapshot("delay control has no expression".into())
            })?;
            Ok(IntraControl::Delay(delay))
        }
        113..=115 => {
            let (specs, implicit) = event_specs(snapshot, timing, ids)?;
            Ok(IntraControl::Event {
                control: timing_id,
                specs,
                implicit,
            })
        }
        116 => {
            let count = edge_target(ids, edges, SemanticEdgeRole::Condition)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated assignment event has no count".into())
            })?;
            let event = edge_target(ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated assignment event has no event control".into())
            })?;
            Ok(IntraControl::Repeat {
                control: timing_id,
                count,
                event: Box::new(intra_control_timing(snapshot, event, ids)?),
            })
        }
        _ => Ok(IntraControl::Unsupported { control: timing_id }),
    }
}

fn timing_statement(
    snapshot: &SlangSnapshot,
    _statement: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<StmtKind, DbError> {
    let timing_id = edge_target(ids, edges, SemanticEdgeRole::Event)?
        .ok_or_else(|| DbError::InvalidSnapshot("timed statement has no timing control".into()))?;
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| DbError::InvalidSnapshot("timing control node is missing".into()))?;
    let timing_edges = semantic_edges(snapshot, timing)?;
    if timing.subkind == 112 {
        let delay = edge_target(ids, timing_edges, SemanticEdgeRole::Delay)?
            .ok_or_else(|| DbError::InvalidSnapshot("delay control has no expression".into()))?;
        return Ok(StmtKind::DelayControl { delay });
    }
    let (specs, implicit) = event_specs(snapshot, timing, ids)?;
    Ok(StmtKind::EventControl {
        specs,
        implicit,
        body: edge_target(ids, edges, SemanticEdgeRole::Body)?,
    })
}

fn event_trigger_timing(
    snapshot: &SlangSnapshot,
    timing_id: NodeId,
    ids: &HashMap<u64, NodeId>,
) -> Result<EventTriggerTiming, DbError> {
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| {
            DbError::InvalidSnapshot("event-trigger timing control node is missing".into())
        })?;
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        112 => {
            let delays: Vec<NodeId> = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Delay)
                .map(|edge| semantic_id(ids, edge.target_id))
                .collect::<Result<_, _>>()?;
            match delays.as_slice() {
                [delay] => Ok(EventTriggerTiming::Delay {
                    control: timing_id,
                    expression: *delay,
                }),
                // Delay3 and 1step controls are retained as unsupported
                // timing nodes rather than degrading to the first operand.
                _ => Ok(EventTriggerTiming::Unsupported { control: timing_id }),
            }
        }
        113..=115 => {
            let (specs, implicit) = event_specs(snapshot, timing, ids)?;
            Ok(EventTriggerTiming::Event {
                control: timing_id,
                specs,
                implicit,
            })
        }
        116 => {
            let count = edge_target(ids, edges, SemanticEdgeRole::Condition)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated event trigger has no count".into())
            })?;
            let event = edge_target(ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated event trigger has no event control".into())
            })?;
            Ok(EventTriggerTiming::Repeat {
                control: timing_id,
                count,
                event: Box::new(event_trigger_timing(snapshot, event, ids)?),
            })
        }
        // Cycle delays and any future bridge timing kinds remain represented
        // by their owned timing node until a simulator implementation exists.
        _ => Ok(EventTriggerTiming::Unsupported { control: timing_id }),
    }
}

fn event_specs(
    snapshot: &SlangSnapshot,
    timing: &SemanticNode,
    ids: &HashMap<u64, NodeId>,
) -> Result<(Vec<EventSpec>, bool), DbError> {
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        113 => {
            let sig = edge_target(ids, edges, SemanticEdgeRole::Event)?
                .ok_or_else(|| DbError::InvalidSnapshot("signal event has no expression".into()))?;
            if let Some(block) = clocking_block_from_expression(snapshot, ids, sig, 0)? {
                let block_node = snapshot
                    .semantic_nodes
                    .get(block.index())
                    .ok_or_else(|| DbError::InvalidSnapshot("clocking block is missing".into()))?;
                let block_event = edge_target(
                    ids,
                    semantic_edges(snapshot, block_node)?,
                    SemanticEdgeRole::Event,
                )?
                .ok_or_else(|| DbError::InvalidSnapshot("clocking block has no event".into()))?;
                let event_node = snapshot
                    .semantic_nodes
                    .get(block_event.index())
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking block event is missing".into())
                    })?;
                return event_specs(snapshot, event_node, ids);
            }
            let named_event = is_named_event_expression(snapshot, ids, sig)?;
            let specs = if timing.is_both_edges {
                vec![
                    EventSpec::Edge { sig, posedge: true },
                    EventSpec::Edge {
                        sig,
                        posedge: false,
                    },
                ]
            } else if timing.is_posedge || timing.is_negedge {
                vec![EventSpec::Edge {
                    sig,
                    posedge: timing.is_posedge,
                }]
            } else if named_event {
                vec![EventSpec::Named(sig)]
            } else {
                vec![EventSpec::AnyChange { sig }]
            };
            let specs =
                if let Some(condition) = edge_target(ids, edges, SemanticEdgeRole::Condition)? {
                    specs
                        .into_iter()
                        .map(|event| EventSpec::Qualified {
                            event: Box::new(event),
                            condition,
                        })
                        .collect()
                } else {
                    specs
                };
            Ok((specs, false))
        }
        114 => {
            let mut specs = Vec::new();
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Event)
            {
                let event_id = semantic_id(ids, edge.target_id)?;
                let event = snapshot
                    .semantic_nodes
                    .get(event_id.index())
                    .ok_or_else(|| DbError::InvalidSnapshot("event list item is missing".into()))?;
                specs.extend(event_specs(snapshot, event, ids)?.0);
            }
            Ok((specs, false))
        }
        115 => Ok((Vec::new(), true)),
        _ => Err(DbError::InvalidSnapshot(format!(
            "unsupported Slang timing control subkind {}",
            timing.subkind
        ))),
    }
}

fn is_named_event_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
) -> Result<bool, DbError> {
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("event expression is missing".into()))?;
    if node.kind == SemanticKind::NamedEvent {
        return Ok(true);
    }
    if node.kind != SemanticKind::Expression {
        return Ok(false);
    }
    match node.subkind {
        65 => node
            .target_id
            .map(|target| semantic_id(ids, target))
            .transpose()?
            .map_or(Ok(false), |target| {
                is_named_event_expression(snapshot, ids, target)
            }),
        73 => {
            let edges = semantic_edges(snapshot, node)?;
            edge_target(ids, edges, SemanticEdgeRole::Base)?.map_or(Ok(false), |base| {
                is_named_event_expression(snapshot, ids, base)
            })
        }
        75 => node
            .target_id
            .map(|target| semantic_id(ids, target))
            .transpose()?
            .map_or(Ok(false), |target| {
                is_named_event_expression(snapshot, ids, target)
            }),
        _ => Ok(false),
    }
}

fn source_spelling(snapshot: &SlangSnapshot, node: &SemanticNode) -> Option<String> {
    // The bridge exports the expanded token for time literals. Never guess a
    // macro's replacement by scanning raw source: inactive branches, undef,
    // includes, and function-like macros make that semantically incorrect.
    if node.kind == SemanticKind::Expression && node.subkind == 85 && !node.name.is_empty() {
        return Some(node.name.clone());
    }
    let range = node.range?;
    let file = snapshot
        .files
        .iter()
        .find(|file| file.id == range.file_id)?;
    let start = usize::try_from(range.start).ok()?;
    let end = usize::try_from(range.end).ok()?;
    let spelling = file.text.get(start..end)?;
    (!spelling.is_empty()).then(|| spelling.to_owned())
}

fn expression_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
    ty: TypeInfo,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    if node.detail == "ArbitrarySymbol" {
        return Ok(NodeKind::Expr(ExprKind::ScopeRef {
            target: required(SemanticEdgeRole::Reference, "scope reference target")?,
        }));
    }
    if node.detail == "DataType" {
        return Ok(NodeKind::Expr(ExprKind::DataType));
    }
    if node.detail == "UnboundedLiteral" {
        return Ok(NodeKind::Expr(ExprKind::Unbounded));
    }
    Ok(NodeKind::Expr(match node.subkind {
        64 | 85 => {
            let value = node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .map(|constant| value_data_from_slang(&constant.value))
                .unwrap_or(ValueData::None);
            let (size, const_type) = match &value {
                ValueData::Vector { bit_width, .. } => (
                    i32::try_from(*bit_width).unwrap_or(i32::MAX),
                    ConstantType::Integer,
                ),
                ValueData::Real(_) if node.subkind == 85 => (64, ConstantType::Time),
                ValueData::Real(_) => (64, ConstantType::Real),
                ValueData::Bytes(_) | ValueData::Str(_) => (-1, ConstantType::String),
                _ => (-1, ConstantType::Null),
            };
            ExprKind::Constant {
                value,
                size,
                const_type,
                source: source_spelling(snapshot, node)
                    .map(ConstantSource::Exact)
                    .unwrap_or(ConstantSource::Unavailable),
                time_scale: (node.subkind == 85)
                    .then(|| time_literal_scale(node.time_scale))
                    .flatten(),
            }
        }
        65 => ExprKind::Ref {
            target: node
                .target_id
                .map(|id| canonical_reference_target(snapshot, ids, id))
                .transpose()?,
        },
        72 => ExprKind::Cast {
            operand: required(SemanticEdgeRole::Operand, "conversion operand")?,
            ty: ty.clone(),
            size_cast: false,
            size_cast_expr: None,
            cast_kind_known: true,
            propagated: node.is_propagated_conversion,
            two_state: snapshot
                .types
                .iter()
                .find(|candidate| Some(candidate.id) == node.type_id)
                .is_some_and(|candidate| !candidate.is_four_state),
        },
        73 => {
            let index = required(SemanticEdgeRole::Index, "element select index")?;
            if let Some((base, indices)) =
                array_select_from_slang(snapshot, type_projector, ids, node, 0)?
            {
                ExprKind::ArraySelect { base, indices }
            } else {
                ExprKind::BitSelect {
                    base: required(SemanticEdgeRole::Base, "element select base")?,
                    index,
                }
            }
        }
        74 if node.is_indexed_up || node.is_indexed_down => ExprKind::IndexedPartSelect {
            base: required(SemanticEdgeRole::Base, "indexed select base")?,
            base_expr: required(SemanticEdgeRole::Left, "indexed select base expression")?,
            width_expr: required(SemanticEdgeRole::Right, "indexed select width")?,
            neg: node.is_indexed_down,
        },
        74 => ExprKind::PartSelect {
            base: required(SemanticEdgeRole::Base, "range select base")?,
            left: required(SemanticEdgeRole::Left, "range select left bound")?,
            right: required(SemanticEdgeRole::Right, "range select right bound")?,
        },
        75 => match member_path_from_slang(snapshot, type_projector, ids, node, 0)? {
            Some((parts, refs)) => ExprKind::HierPath { parts, refs },
            None => ExprKind::Other,
        },
        86 => ExprKind::NewArray {
            size: required(SemanticEdgeRole::Width, "dynamic-array size")?,
            initializer: first(SemanticEdgeRole::Initializer)?,
        },
        87 => ExprKind::NewClass {
            class_name: ty.type_name.clone(),
            class_type: node.type_id.map(TypeId),
            constructor: first(SemanticEdgeRole::Initializer)?,
            is_super_class: node.auxiliary & crate::ffi::slang::NEW_CLASS_SUPER != 0,
        },
        90 => {
            let target = node
                .target_id
                .map(|id| semantic_id(ids, id))
                .transpose()?
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("assertion instance has no target".into())
                })?;
            let body = required(SemanticEdgeRole::Body, "assertion instance body")?;
            let mut formals = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::AssertionFormal)
                .collect::<Vec<_>>();
            let mut actuals = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::AssertionActual)
                .collect::<Vec<_>>();
            formals.sort_by_key(|edge| edge.index);
            actuals.sort_by_key(|edge| edge.index);
            if formals.len() != actuals.len()
                || formals
                    .iter()
                    .zip(&actuals)
                    .any(|(formal, actual)| formal.index != actual.index)
            {
                return Err(DbError::InvalidSnapshot(
                    "assertion instance formal/actual bindings are not paired".into(),
                ));
            }
            let bindings = formals
                .into_iter()
                .zip(actuals)
                .map(|(formal, actual)| {
                    Ok(AssertionBinding {
                        formal: semantic_id(ids, formal.target_id)?,
                        actual: semantic_id(ids, actual.target_id)?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            ExprKind::AssertionInstance {
                target,
                body,
                bindings,
            }
        }
        SEMANTIC_EXPR_CLOCKING_EVENT => {
            let control = required(SemanticEdgeRole::Operand, "clocking event control")?;
            let timing = snapshot
                .semantic_nodes
                .get(control.index())
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("clocking event control is missing".into())
                })?;
            if timing.kind != SemanticKind::TimingControl
                || timing.subkind != 113
                || (!timing.is_posedge && !timing.is_negedge)
            {
                ExprKind::Other
            } else {
                let timing_edges = semantic_edges(snapshot, timing)?;
                let signal =
                    edge_target(ids, timing_edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking event has no signal".into())
                    })?;
                if is_named_event_expression(snapshot, ids, signal)? {
                    ExprKind::Other
                } else {
                    let gate = edge_target(ids, timing_edges, SemanticEdgeRole::Condition)?;
                    ExprKind::ClockingEvent {
                        signal,
                        posedge: timing.is_posedge,
                        gate,
                    }
                }
            }
        }
        81..=84 => {
            let key_type = if node.subkind == 82 {
                let type_id = node.type_id.ok_or_else(|| {
                    DbError::InvalidSnapshot("assignment pattern type key has no type".into())
                })?;
                let projection = type_projector.project(type_id)?;
                Some(AssignmentPatternKeyType {
                    type_id: projection.descriptor.id,
                    ty: projection.type_info,
                    two_state: projection.two_state,
                    packed_ranges: projection.packed_dimensions,
                })
            } else {
                None
            };
            ExprKind::TaggedPattern {
                key: (!node.name.is_empty()).then(|| node.name.clone()),
                key_type,
                value: first(SemanticEdgeRole::Body)?,
            }
        }
        69 if matches!(
            node.operation,
            SemanticOperation::StreamLeft | SemanticOperation::StreamRight
        ) =>
        {
            let mut operand_edges = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect::<Vec<_>>();
            operand_edges.sort_by_key(|edge| edge.index);
            let operand_indices = operand_edges
                .iter()
                .map(|edge| edge.index)
                .collect::<HashSet<_>>();
            let streams = operand_edges
                .into_iter()
                .map(|operand| {
                    let value = semantic_id(ids, operand.target_id)?;
                    let with_expr = edges
                        .iter()
                        .find(|edge| {
                            edge.role == SemanticEdgeRole::Index && edge.index == operand.index
                        })
                        .map(|edge| semantic_id(ids, edge.target_id))
                        .transpose()?;
                    Ok(StreamOperand { value, with_expr })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            if edges.iter().any(|edge| {
                edge.role == SemanticEdgeRole::Index && !operand_indices.contains(&edge.index)
            }) {
                return Err(DbError::InvalidSnapshot(
                    "streaming selector has no matching operand".into(),
                ));
            }
            ExprKind::Streaming {
                direction: if node.operation == SemanticOperation::StreamLeft {
                    StreamingDirection::LeftToRight
                } else {
                    StreamingDirection::RightToLeft
                },
                slice_size: node.auxiliary,
                streams,
            }
        }
        _ if node.operation != SemanticOperation::None => {
            let operands = match node.subkind {
                66 => edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
                67 => [SemanticEdgeRole::Left, SemanticEdgeRole::Right]
                    .into_iter()
                    .map(|role| required(role, "binary operand"))
                    .collect::<Result<Vec<_>, _>>()?,
                68 => {
                    let mut values = edge_targets(ids, edges, SemanticEdgeRole::Condition)?;
                    values.push(required(
                        SemanticEdgeRole::Then,
                        "conditional true operand",
                    )?);
                    values.push(required(
                        SemanticEdgeRole::Else,
                        "conditional false operand",
                    )?);
                    values
                }
                70 => {
                    let mut values = vec![required(SemanticEdgeRole::Width, "replication count")?];
                    values.extend(edge_targets(ids, edges, SemanticEdgeRole::Operand)?);
                    values
                }
                71 => [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| required(role, "assignment operand"))
                    .collect::<Result<Vec<_>, _>>()?,
                77 => {
                    let mut values = vec![required(SemanticEdgeRole::Lhs, "inside selector")?];
                    values.extend(edge_targets(ids, edges, SemanticEdgeRole::Operand)?);
                    values
                }
                89 => [SemanticEdgeRole::Left, SemanticEdgeRole::Right]
                    .into_iter()
                    .map(|role| required(role, "value range bound"))
                    .collect::<Result<Vec<_>, _>>()?,
                _ => edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
            };
            ExprKind::Operation {
                op: operation_from_slang(node.operation, node.subkind == 66),
                reordered: false,
                assignment: node.subkind == 71,
                operands,
            }
        }
        _ => ExprKind::Other,
    }))
}

fn source_position(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
) -> Result<(Option<String>, u32, u32, u32, u32), DbError> {
    let Some(range) = node.range else {
        return Ok((None, 0, 0, 0, 0));
    };
    let file = snapshot
        .files
        .iter()
        .find(|file| file.id == range.file_id)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic range file is missing".into()))?;
    let start = usize::try_from(range.start)
        .map_err(|_| DbError::InvalidSnapshot("semantic range start is too large".into()))?;
    let end = usize::try_from(range.end)
        .map_err(|_| DbError::InvalidSnapshot("semantic range end is too large".into()))?;
    let (line, col) = line_column(&file.text, start)?;
    let (end_line, end_col) = line_column(&file.text, end)?;
    Ok((Some(file.name.clone()), line, col, end_line, end_col))
}

fn line_column(text: &str, offset: usize) -> Result<(u32, u32), DbError> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err(DbError::InvalidSnapshot(
            "semantic range is not on a source character boundary".into(),
        ));
    }
    let prefix = &text[..offset];
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)
        .map_err(|_| DbError::InvalidSnapshot("source line number is too large".into()))?;
    let column = u32::try_from(offset - line_start + 1)
        .map_err(|_| DbError::InvalidSnapshot("source column is too large".into()))?;
    Ok((line, column))
}

fn semantic_full_name(nodes: &[Node], id: NodeId) -> Result<String, DbError> {
    let mut parts = Vec::new();
    let mut current = Some(id);
    let mut visited = HashSet::new();
    while let Some(node_id) = current {
        if !visited.insert(node_id) {
            return Err(DbError::InvalidSnapshot(
                "Slang semantic parent links contain a cycle".into(),
            ));
        }
        let node = nodes.get(node_id.index()).ok_or_else(|| {
            DbError::InvalidSnapshot("Slang semantic parent is outside the node arena".into())
        })?;
        if !node.name.is_empty() {
            parts.push(node.name.as_str());
        }
        current = node.parent;
    }
    parts.reverse();
    Ok(parts.join("."))
}

fn enclosing_scope_name(nodes: &[Node], id: NodeId) -> Option<String> {
    let parent = nodes.get(id.index())?.parent?;
    let full_name = &nodes.get(parent.index())?.full_name;
    (!full_name.is_empty()).then(|| full_name.clone())
}

impl Db {
    #[cfg(test)]
    pub(super) fn empty_for_validation_test() -> Self {
        Self {
            nodes: Vec::new(),
            edition: LanguageEdition::SystemVerilog2009,
            overridden_parameters: HashSet::new(),
            semantic_kinds: Vec::new(),
            semantic_details: Vec::new(),
            program_instances: HashSet::new(),
            tops: Vec::new(),
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            class_metadata: HashMap::new(),
            design_name: "test".to_owned(),
            arrays: HashMap::new(),
            event_arrays: HashMap::new(),
            array_select_paths: HashMap::new(),
            vars_init: HashMap::new(),
            net_delays: HashMap::new(),
            var_lifetimes: HashMap::new(),
            var_lifetime_qualifiers: HashMap::new(),
            method_calls_with_clause: HashSet::new(),
            method_call_iterators: HashMap::new(),
            packed_members: HashMap::new(),
            aggregate_layouts: HashMap::new(),
            type_descriptors: HashMap::new(),
            enum_types: HashMap::new(),
            packed_dimensions: HashMap::new(),
            two_state_types: HashSet::new(),
            clocking_blocks: HashMap::new(),
            clocking_vars: HashMap::new(),
            modport_directions: HashMap::new(),
            virtual_interface_targets: HashMap::new(),
            dpi_imports: HashMap::new(),
            implicit_nets: HashSet::new(),
            implicit_conversions: HashSet::new(),
            source_files: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn push_top_for_validation_test(&mut self, id: NodeId) {
        self.tops.push(id);
    }

    /// Construct a semantic database directly for tests of downstream IR
    /// layers. The same structural validator used for frontend snapshots runs
    /// before the database is returned.
    #[cfg(test)]
    pub(crate) fn from_test_nodes(
        design_name: impl Into<String>,
        nodes: Vec<Node>,
        tops: Vec<NodeId>,
        arrays: HashMap<NodeId, ArrayMeta>,
    ) -> Result<Self, DbError> {
        let db = Self {
            nodes,
            edition: LanguageEdition::SystemVerilog2009,
            overridden_parameters: HashSet::new(),
            semantic_kinds: Vec::new(),
            semantic_details: Vec::new(),
            program_instances: HashSet::new(),
            tops,
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            class_metadata: HashMap::new(),
            design_name: design_name.into(),
            arrays,
            event_arrays: HashMap::new(),
            array_select_paths: HashMap::new(),
            vars_init: HashMap::new(),
            net_delays: HashMap::new(),
            var_lifetimes: HashMap::new(),
            var_lifetime_qualifiers: HashMap::new(),
            method_calls_with_clause: HashSet::new(),
            method_call_iterators: HashMap::new(),
            packed_members: HashMap::new(),
            aggregate_layouts: HashMap::new(),
            type_descriptors: HashMap::new(),
            enum_types: HashMap::new(),
            packed_dimensions: HashMap::new(),
            two_state_types: HashSet::new(),
            clocking_blocks: HashMap::new(),
            clocking_vars: HashMap::new(),
            modport_directions: HashMap::new(),
            virtual_interface_targets: HashMap::new(),
            dpi_imports: HashMap::new(),
            implicit_nets: HashSet::new(),
            implicit_conversions: HashSet::new(),
            source_files: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
        };
        db.validate().map_err(DbError::InvalidDatabase)?;
        Ok(db)
    }

    /// Build the owned semantic database from a validated Slang snapshot.
    ///
    /// This conversion never reads source files and retains no native owner.
    pub fn from_slang(snapshot: &SlangSnapshot) -> Result<Self, DbError> {
        let type_projector = SlangTypeProjector::new(snapshot)?;
        let ids = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, NodeId::from_index(index)))
            .collect::<HashMap<_, _>>();
        if ids.len() != snapshot.semantic_nodes.len() {
            return Err(DbError::InvalidSnapshot(
                "duplicate Slang semantic node id".to_owned(),
            ));
        }
        if snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .any(|(index, node)| node.id != index as u64)
        {
            return Err(DbError::InvalidSnapshot(
                "Slang semantic node ids are not contiguous arena indices".to_owned(),
            ));
        }

        let mut nodes = Vec::with_capacity(snapshot.semantic_nodes.len());
        let overridden_parameters = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                semantic.kind == SemanticKind::Parameter && semantic.auxiliary == 1
            })
            .map(|(index, _)| NodeId::from_index(index))
            .collect();
        let semantic_kinds = snapshot
            .semantic_nodes
            .iter()
            .map(|semantic| semantic.kind.into())
            .collect();
        let semantic_details = snapshot
            .semantic_nodes
            .iter()
            .map(|semantic| semantic.detail.clone())
            .collect();
        let program_instances = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                matches!(
                    semantic.kind,
                    SemanticKind::Instance | SemanticKind::Definition
                ) && semantic.definition_kind == Some(SemanticDefinitionKind::Program)
            })
            .map(|(index, _)| NodeId::from_index(index))
            .collect();
        let mut arrays = HashMap::new();
        let mut event_arrays = HashMap::new();
        let mut array_select_paths = HashMap::new();
        let mut vars_init = HashMap::new();
        let mut net_delays = HashMap::new();
        let mut two_state_types = HashSet::new();
        let mut implicit_nets = HashSet::new();
        let mut implicit_conversions = HashSet::new();
        let mut var_lifetimes = HashMap::new();
        let mut var_lifetime_qualifiers = HashMap::new();
        let mut method_calls_with_clause = HashSet::new();
        let mut method_call_iterators = HashMap::new();
        let mut packed_members = HashMap::new();
        let mut aggregate_layouts = HashMap::new();
        let mut type_descriptors = HashMap::new();
        let mut enum_types = HashMap::new();
        let mut packed_dimensions = HashMap::new();
        let mut clocking_blocks = HashMap::new();
        let mut clocking_vars = HashMap::new();
        let dpi_imports = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                semantic.kind == SemanticKind::Subroutine
                    && semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_IMPORT != 0
            })
            .map(|semantic| {
                let id = ids[&semantic.id];
                let c_name = if semantic.definition_name.is_empty() {
                    semantic.name.clone()
                } else {
                    semantic.definition_name.clone()
                };
                (
                    id,
                    DpiImportInfo {
                        c_name,
                        context: semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_CONTEXT
                            != 0,
                        pure: semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_PURE != 0,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        for semantic in &snapshot.semantic_nodes {
            let id = ids[&semantic.id];
            let edges = semantic_edges(snapshot, semantic)?;
            let mut children = Vec::new();
            for edge in edges {
                let child_semantic = snapshot
                    .semantic_nodes
                    .get(edge.target_id as usize)
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("semantic child target is missing".into())
                    })?;
                if child_semantic.is_uninstantiated
                    || child_semantic.kind == SemanticKind::Definition
                {
                    continue;
                }
                if !matches!(
                    edge.role,
                    SemanticEdgeRole::Reference
                        | SemanticEdgeRole::Callee
                        | SemanticEdgeRole::Actual
                        | SemanticEdgeRole::HighConnection
                        | SemanticEdgeRole::LowConnection
                        | SemanticEdgeRole::SourceIdentity
                        | SemanticEdgeRole::ReturnOwner
                ) {
                    children.push(semantic_id(&ids, edge.target_id)?);
                }
            }
            if semantic.kind == SemanticKind::Port {
                if let Some(high_expression) =
                    edge_target(&ids, edges, SemanticEdgeRole::HighConnection)?
                {
                    children.push(high_expression);
                }
            }
            if semantic.kind == SemanticKind::Instance {
                let mut flattened = Vec::new();
                for child in children {
                    let child_semantic = &snapshot.semantic_nodes[child.index()];
                    if child_semantic.kind == SemanticKind::Scope && child_semantic.subkind == 194 {
                        flattened.extend(
                            semantic_edges(snapshot, child_semantic)?
                                .iter()
                                .filter(|edge| edge.role != SemanticEdgeRole::Reference)
                                .filter(|edge| {
                                    snapshot
                                        .semantic_nodes
                                        .get(edge.target_id as usize)
                                        .is_some_and(|node| {
                                            !node.is_uninstantiated
                                                && node.kind != SemanticKind::Definition
                                        })
                                })
                                .map(|edge| semantic_id(&ids, edge.target_id))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    } else if child_semantic.kind == SemanticKind::Instance
                        && child_semantic.subkind == 193
                    {
                        flattened.extend(
                            semantic_edges(snapshot, child_semantic)?
                                .iter()
                                .filter(|edge| edge.role == SemanticEdgeRole::Child)
                                .filter(|edge| {
                                    snapshot
                                        .semantic_nodes
                                        .get(edge.target_id as usize)
                                        .is_some_and(|node| {
                                            !node.is_uninstantiated
                                                && node.kind != SemanticKind::Definition
                                        })
                                })
                                .map(|edge| semantic_id(&ids, edge.target_id))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    } else {
                        flattened.push(child);
                    }
                }
                children = flattened;
            } else if semantic.kind == SemanticKind::Scope && semantic.subkind == 194 {
                children.clear();
            }
            let mut seen_children = HashSet::new();
            children.retain(|child| seen_children.insert(*child));
            let projection = semantic
                .type_id
                .map(|type_id| type_projector.project(type_id))
                .transpose()?;
            let type_info = projection
                .as_ref()
                .map(|projection| projection.type_info.clone())
                .unwrap_or_default();
            if projection
                .as_ref()
                .is_some_and(|projection| projection.two_state)
            {
                two_state_types.insert(id);
            }
            if semantic.kind == SemanticKind::Net && semantic.is_implicit {
                implicit_nets.insert(id);
            }
            if semantic.is_implicit_conversion {
                implicit_conversions.insert(id);
            }
            if semantic.kind == SemanticKind::MethodCall && semantic.method_with_clause {
                method_calls_with_clause.insert(id);
                if let Some(iterator) = semantic.target_id {
                    method_call_iterators.insert(id, semantic_id(&ids, iterator)?);
                }
            }
            if semantic.kind == SemanticKind::Scope
                && semantic.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
            {
                let event =
                    edge_target(&ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking block has no event control".into())
                    })?;
                let event_node = snapshot.semantic_nodes.get(event.index()).ok_or_else(|| {
                    DbError::InvalidSnapshot("clocking block event is missing".into())
                })?;
                let (event_specs, event_implicit) = event_specs(snapshot, event_node, &ids)?;
                let input_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 0)?;
                let output_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 1)?;
                let default_input = clocking_skew_from_slang(
                    snapshot,
                    &ids,
                    input_delay,
                    semantic.auxiliary >> CLOCKING_INPUT_EDGE_SHIFT,
                )?;
                let default_output = clocking_skew_from_slang(
                    snapshot,
                    &ids,
                    output_delay,
                    semantic.auxiliary >> CLOCKING_OUTPUT_EDGE_SHIFT,
                )?;
                clocking_blocks.insert(
                    id,
                    ClockingBlockInfo {
                        event,
                        event_specs,
                        event_implicit,
                        is_default: semantic.auxiliary & CLOCKING_BLOCK_DEFAULT != 0,
                        is_global: semantic.auxiliary & CLOCKING_BLOCK_GLOBAL != 0,
                        default_input,
                        default_output,
                    },
                );
            }
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind == SEMANTIC_VARIABLE_CLOCKING
            {
                let initializer = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "clocking variable has no source expression".into(),
                        )
                    })?;
                let source = clocking_source_from_expression(snapshot, &ids, initializer, 0)?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking variable source is unresolved".into())
                    })?;
                let parent_raw = semantic
                    .parent_id
                    .and_then(|parent| snapshot.semantic_nodes.get(parent as usize));
                let block = semantic
                    .parent_id
                    .and_then(|parent| ids.get(&parent).copied())
                    .filter(|_| {
                        parent_raw.is_some_and(|parent| {
                            parent.kind == SemanticKind::Scope
                                && parent.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
                        })
                    })
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "clocking variable is not owned by a clocking block".into(),
                        )
                    })?;
                let input_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 0)?;
                let output_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 1)?;
                clocking_vars.insert(
                    id,
                    ClockingVarInfo {
                        block,
                        source,
                        direction: direction_from_slang(semantic),
                        input: clocking_skew_from_slang(
                            snapshot,
                            &ids,
                            input_delay,
                            semantic.auxiliary,
                        )?,
                        output: clocking_skew_from_slang(
                            snapshot,
                            &ids,
                            output_delay,
                            semantic.auxiliary >> CLOCKING_VAR_OUTPUT_EDGE_SHIFT,
                        )?,
                    },
                );
            }
            if matches!(
                semantic.kind,
                SemanticKind::Variable | SemanticKind::NamedEvent
            ) && semantic.subkind != 229
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
            {
                let resolved_lifetime = match semantic.auxiliary {
                    0 => VariableLifetime::Unavailable,
                    1 => VariableLifetime::Static,
                    2 => VariableLifetime::Automatic,
                    _ => {
                        return Err(DbError::InvalidSnapshot(
                            "variable has an unknown resolved lifetime".into(),
                        ));
                    }
                };
                var_lifetimes.insert(id, resolved_lifetime);
                let lifetime = if semantic.is_automatic {
                    VariableLifetimeQualifier::Automatic
                } else if semantic.is_static {
                    VariableLifetimeQualifier::Static
                } else {
                    VariableLifetimeQualifier::None
                };
                var_lifetime_qualifiers.insert(id, lifetime);
            }
            let is_event_array = semantic.kind == SemanticKind::NamedEvent
                && projection
                    .as_ref()
                    .is_some_and(|projection| projection.array.is_some());
            let is_array = matches!(semantic.kind, SemanticKind::Variable | SemanticKind::Net)
                && semantic.subkind != 229
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
                && projection
                    .as_ref()
                    .is_some_and(|projection| projection.array.is_some())
                || semantic.kind == SemanticKind::Array;
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
                && !is_array
            {
                if let Some(initializer) = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                {
                    vars_init.insert(id, initializer);
                }
            }
            if is_array {
                let array = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref());
                arrays.insert(
                    id,
                    ArrayMeta {
                        kind: array
                            .map(|array| array.kind.clone())
                            .unwrap_or(ArrayKind::Static),
                        dims: array
                            .map(|array| array.dimensions.clone())
                            .unwrap_or_default(),
                        init: edge_target(&ids, edges, SemanticEdgeRole::Initializer)?,
                        net_type: (semantic.kind == SemanticKind::Net)
                            .then(|| net_type_from_subkind(semantic.subkind)),
                    },
                );
            }
            if is_event_array {
                let array = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref());
                event_arrays.insert(
                    id,
                    ArrayMeta {
                        kind: array
                            .map(|array| array.kind.clone())
                            .unwrap_or(ArrayKind::Static),
                        dims: array
                            .map(|array| array.dimensions.clone())
                            .unwrap_or_default(),
                        init: edge_target(&ids, edges, SemanticEdgeRole::Initializer)?,
                        net_type: None,
                    },
                );
            }
            if let Some(projection) = &projection {
                type_descriptors.insert(id, projection.descriptor.clone());
                if !projection.packed_dimensions.is_empty() {
                    packed_dimensions.insert(id, projection.packed_dimensions.clone());
                }
                if let Some(members) = &projection.packed_members {
                    packed_members.insert(id, members.clone());
                }
                if let Some(layout) = &projection.aggregate_layout {
                    aggregate_layouts.insert(id, layout.clone());
                }
            }
            if semantic.kind == SemanticKind::EnumConstant {
                if let (Some(type_id), Some(Val::Bits(value)), Some(enum_type)) = (
                    semantic.type_id,
                    semantic
                        .constant_id
                        .and_then(|constant_id| snapshot.constants.get(constant_id as usize))
                        .and_then(|constant| val_from_slang(&constant.value)),
                    projection.as_ref(),
                ) {
                    if enum_type.type_info.kind == "enum" {
                        let width = enum_type.type_info.width.ok_or_else(|| {
                            DbError::InvalidSnapshot(format!(
                                "enum type {type_id} has no resolved width"
                            ))
                        })?;
                        enum_types
                            .entry(TypeId(type_id))
                            .or_insert_with(|| EnumTypeMetadata {
                                width,
                                signed: enum_type.type_info.signed,
                                two_state: enum_type.two_state,
                                members: Vec::new(),
                            })
                            .members
                            .push(EnumMember {
                                name: semantic.name.clone(),
                                value: Val::Bits(value),
                            });
                    }
                }
            }
            let mut parent = if semantic.is_top
                || matches!(
                    semantic.kind,
                    SemanticKind::Definition | SemanticKind::Package | SemanticKind::Class
                ) {
                None
            } else {
                semantic
                    .parent_id
                    .map(|parent| semantic_id(&ids, parent))
                    .transpose()?
            };
            if let Some(parent_id) = parent {
                let parent_semantic = &snapshot.semantic_nodes[parent_id.index()];
                if parent_semantic.kind == SemanticKind::Scope && parent_semantic.subkind == 194 {
                    parent = parent_semantic
                        .parent_id
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                } else if parent_semantic.kind == SemanticKind::Instance
                    && parent_semantic.subkind == 193
                {
                    parent = parent_semantic
                        .parent_id
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                }
            }
            let (file, line, col, end_line, end_col) = source_position(snapshot, semantic)?;
            let mut kind =
                node_kind_from_slang(snapshot, &type_projector, semantic, edges, &ids, type_info)?;
            if semantic.kind == SemanticKind::Net {
                if let Some(delay) = driver_delay(snapshot, &ids, edges)? {
                    net_delays.insert(id, delay);
                }
            }
            if semantic.kind == SemanticKind::Expression && semantic.subkind == 73 {
                if let Some(base) = edge_target(&ids, edges, SemanticEdgeRole::Base)? {
                    if let Some((parts, refs)) = member_path_from_slang(
                        snapshot,
                        &type_projector,
                        &ids,
                        &snapshot.semantic_nodes[base.index()],
                        0,
                    )? {
                        if parts.len() > 1 {
                            if let Some(owner) = refs.into_iter().flatten().next() {
                                array_select_paths
                                    .insert(id, (owner, parts.into_iter().skip(1).collect()));
                            }
                        }
                    }
                }
            }
            if is_array {
                let element_type = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref())
                    .map(|array| array.element_type.clone())
                    .unwrap_or_default();
                kind = NodeKind::Array { ty: element_type };
            }
            if matches!(kind, NodeKind::Stmt(StmtKind::Assign { .. })) {
                let expression =
                    edge_target(&ids, edges, SemanticEdgeRole::Body)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("assignment statement has no expression".into())
                    })?;
                let expression_node = &snapshot.semantic_nodes[expression.index()];
                let expression_edges = semantic_edges(snapshot, expression_node)?;
                children = [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| {
                        edge_target(&ids, expression_edges, role)?.ok_or_else(|| {
                            DbError::InvalidSnapshot("assignment expression has no operand".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            } else if semantic.kind == SemanticKind::ContinuousAssign {
                let assignment =
                    edge_target(&ids, edges, SemanticEdgeRole::Body)?.ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "continuous assignment has no assignment expression".into(),
                        )
                    })?;
                let assignment_node = &snapshot.semantic_nodes[assignment.index()];
                let assignment_edges = semantic_edges(snapshot, assignment_node)?;
                children = [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| {
                        edge_target(&ids, assignment_edges, role)?.ok_or_else(|| {
                            DbError::InvalidSnapshot(
                                "continuous assignment expression has no operand".into(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(delay) = edge_target(&ids, edges, SemanticEdgeRole::Delay)? {
                    children.push(delay);
                }
            } else if semantic.kind == SemanticKind::Statement && semantic.subkind == 40 {
                children = match &kind {
                    NodeKind::Stmt(StmtKind::EventControl { specs, body, .. }) => {
                        let mut values = Vec::new();
                        for spec in specs {
                            spec.referenced_nodes(&mut values);
                        }
                        values.extend(*body);
                        values
                    }
                    _ => edge_target(&ids, edges, SemanticEdgeRole::Body)?
                        .into_iter()
                        .collect(),
                };
            } else if semantic.kind == SemanticKind::Statement
                && semantic.subkind == 42
                && semantic.auxiliary == 1
            {
                children = match &kind {
                    NodeKind::Stmt(StmtKind::WaitOrder {
                        events,
                        if_true,
                        if_false,
                    }) => {
                        let mut values = events.clone();
                        values.extend(if_true.iter().chain(if_false.iter()).copied());
                        values
                    }
                    _ => Vec::new(),
                };
            }
            let target_name = || {
                semantic
                    .target_id
                    .and_then(|target| ids.get(&target))
                    .and_then(|target| snapshot.semantic_nodes.get(target.index()))
                    .map(|target| target.name.clone())
                    .unwrap_or_default()
            };
            let name = if semantic.name.is_empty()
                && ((semantic.kind == SemanticKind::Statement
                    && matches!(semantic.subkind, 32 | 56..=58))
                    || (semantic.kind == SemanticKind::Expression && semantic.subkind == 65))
            {
                target_name()
            } else {
                semantic.name.clone()
            };
            nodes.push(Node {
                kind,
                children,
                parent,
                name,
                full_name: String::new(),
                file,
                line,
                col,
                end_line,
                end_col,
            });
        }

        // Member-access expressions through a virtual interface can bind the
        // final clocking variable to a detached semantic node. Slang retains
        // the declaration's `ClockVar` detail and source range on that node,
        // but not the declaration subkind used above. Reuse the declaration
        // metadata by range/name so lowering can select sampled storage for
        // both static and dynamically-held virtual interfaces.
        let clocking_declarations = clocking_vars
            .iter()
            .filter_map(|(id, info)| {
                let semantic = snapshot.semantic_nodes.get(id.index())?;
                Some((*id, semantic.range?, semantic.name.clone(), info.clone()))
            })
            .collect::<Vec<_>>();
        for semantic in &snapshot.semantic_nodes {
            if semantic.detail != "ClockVar" {
                continue;
            }
            let id = ids[&semantic.id];
            if clocking_vars.contains_key(&id) {
                continue;
            }
            let Some(range) = semantic.range else {
                continue;
            };
            if let Some((_, _, _, info)) =
                clocking_declarations
                    .iter()
                    .find(|(_, declaration_range, name, _)| {
                        *declaration_range == range && name == &semantic.name
                    })
            {
                clocking_vars.insert(id, info.clone());
            }
        }

        // A virtual interface handle is an elaboration-time alias to a
        // concrete interface instance.  Capture that static binding while
        // the frontend identities are still available; lowering can then
        // resolve clocking members without retaining native Slang objects.
        let modport_directions = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                semantic.kind == SemanticKind::Modport && semantic.detail == "ModportPort"
            })
            .map(|(index, semantic)| (NodeId::from_index(index), direction_from_slang(semantic)))
            .collect();
        let mut virtual_interface_targets = HashMap::new();
        for &variable in vars_init.keys() {
            if let Some(instance) = virtual_interface_instance_from_slang(snapshot, &ids, variable)?
            {
                virtual_interface_targets.insert(variable, instance);
            }
        }

        for index in 0..nodes.len() {
            let full_name = semantic_full_name(&nodes, NodeId::from_index(index))?;
            nodes[index].full_name = full_name;
        }
        let mut elaborated_type_ranges = Vec::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let id = NodeId::from_index(index);
            let Some(type_id) = semantic.type_id else {
                continue;
            };
            if semantic.name.is_empty()
                || !matches!(
                    semantic.kind,
                    SemanticKind::Net | SemanticKind::Variable | SemanticKind::Array
                )
                || !packed_dimensions.contains_key(&id)
            {
                continue;
            }
            let mut ancestor = semantic.parent_id;
            let mut has_runtime_instance = false;
            for _ in 0..snapshot.semantic_nodes.len() {
                let Some(parent) = ancestor.and_then(|id| snapshot.semantic_nodes.get(id as usize))
                else {
                    break;
                };
                if parent.kind == SemanticKind::Instance && parent.subkind == 192 {
                    has_runtime_instance = true;
                    break;
                }
                ancestor = parent.parent_id;
            }
            if !has_runtime_instance {
                continue;
            }
            let instance = enclosing_scope_name(&nodes, id)
                .unwrap_or_else(|| nodes[id.index()].full_name.clone());
            if !instance.is_empty() {
                elaborated_type_ranges.push(type_projector.elaborated_ranges(
                    id,
                    instance,
                    semantic.name.clone(),
                    type_id,
                )?);
            }
        }
        let tops: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Instance && node.is_top)
            .map(|node| ids[&node.id])
            .collect();
        let flat_modules: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Definition)
            .map(|node| ids[&node.id])
            .collect();
        let packages: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Package)
            .map(|node| ids[&node.id])
            .collect();
        let classes: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| ids[&node.id])
            .collect();
        let class_metadata = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| -> Result<(NodeId, ClassMetadata), DbError> {
                let id = ids[&node.id];
                Ok((
                    id,
                    ClassMetadata {
                        type_id: node.type_id.map(TypeId),
                        base: node.target_id.and_then(|target| ids.get(&target).copied()),
                        base_constructor: edge_target(
                            &ids,
                            semantic_edges(snapshot, node)?,
                            SemanticEdgeRole::BaseConstructor,
                        )?,
                        is_abstract: node.auxiliary & crate::ffi::slang::CLASS_ABSTRACT != 0,
                        is_final: node.auxiliary & crate::ffi::slang::CLASS_FINAL != 0,
                        is_interface: node.auxiliary & crate::ffi::slang::CLASS_INTERFACE != 0,
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        let design_name = tops
            .first()
            .map(|id| nodes[id.index()].name.clone())
            .unwrap_or_else(|| "design".to_owned());
        let db = Self {
            nodes,
            edition: snapshot.edition(),
            overridden_parameters,
            semantic_kinds,
            semantic_details,
            program_instances,
            tops,
            flat_modules,
            packages,
            classes,
            class_metadata,
            design_name,
            arrays,
            event_arrays,
            array_select_paths,
            vars_init,
            net_delays,
            var_lifetimes,
            var_lifetime_qualifiers,
            method_calls_with_clause,
            method_call_iterators,
            packed_members,
            aggregate_layouts,
            type_descriptors,
            enum_types,
            packed_dimensions,
            two_state_types,
            clocking_blocks,
            clocking_vars,
            modport_directions,
            virtual_interface_targets,
            dpi_imports,
            implicit_nets,
            implicit_conversions,
            source_files: snapshot
                .files
                .iter()
                .map(|file| (file.name.clone(), file.text.clone()))
                .collect(),
            elaborated_type_ranges,
        };
        db.validate().map_err(DbError::InvalidDatabase)?;
        Ok(db)
    }

    /// The node at `id`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    /// Checked lookup for IDs that may have originated outside this database.
    pub fn try_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// The kind of the node at `id`.
    pub fn node_kind(&self, id: NodeId) -> &NodeKind {
        &self.nodes[id.index()].kind
    }

    /// Every arena node in stable capture order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Every arena ID in stable capture order.
    pub fn node_ids(&self) -> impl ExactSizeIterator<Item = NodeId> + '_ {
        (0..self.nodes.len()).map(NodeId::from_index)
    }

    /// Native semantic category retained for nodes imported from Slang.
    /// Synthetic test databases do not have a native category and return
    /// `None`.
    pub fn semantic_kind(&self, id: NodeId) -> Option<CapturedSemanticKind> {
        self.semantic_kinds.get(id.index()).copied()
    }

    /// Native detail retained for diagnostics about a captured node.
    pub fn semantic_detail(&self, id: NodeId) -> Option<&str> {
        self.semantic_details.get(id.index()).map(String::as_str)
    }

    /// Whether an elaborated instance or definition has program-block
    /// semantics.  Synthetic test databases do not carry native definition
    /// metadata and therefore report `false`.
    pub fn is_program_instance(&self, id: NodeId) -> bool {
        self.program_instances.contains(&id)
    }

    pub(crate) fn semantic_metadata_lengths(&self) -> (usize, usize) {
        (self.semantic_kinds.len(), self.semantic_details.len())
    }

    pub fn tops(&self) -> &[NodeId] {
        &self.tops
    }

    pub fn flat_modules(&self) -> &[NodeId] {
        &self.flat_modules
    }

    pub fn packages(&self) -> &[NodeId] {
        &self.packages
    }

    pub fn classes(&self) -> &[NodeId] {
        &self.classes
    }

    /// Return the owned inheritance/type metadata for a class node.
    pub fn class_metadata(&self, id: NodeId) -> Option<&ClassMetadata> {
        self.class_metadata.get(&id)
    }

    pub(crate) fn class_metadata_entries(&self) -> &HashMap<NodeId, ClassMetadata> {
        &self.class_metadata
    }

    /// Resolve a canonical frontend class type id to its owned class node.
    pub fn class_for_type(&self, type_id: TypeId) -> Option<NodeId> {
        self.class_metadata
            .iter()
            .find_map(|(node, metadata)| (metadata.type_id == Some(type_id)).then_some(*node))
    }

    pub fn design_name(&self) -> &str {
        &self.design_name
    }

    /// Return the owned DPI-C import contract for a subroutine declaration.
    pub fn dpi_import(&self, id: NodeId) -> Option<&DpiImportInfo> {
        self.dpi_imports.get(&id)
    }

    pub fn arrays(&self) -> &HashMap<NodeId, ArrayMeta> {
        &self.arrays
    }

    pub fn array_meta(&self, id: NodeId) -> Option<&ArrayMeta> {
        self.arrays.get(&id)
    }

    /// Return unpacked-array metadata for a named-event declaration, when the
    /// declaration has an unpacked event-array type.
    pub fn event_arrays(&self) -> &HashMap<NodeId, ArrayMeta> {
        &self.event_arrays
    }

    pub fn event_array_meta(&self, id: NodeId) -> Option<&ArrayMeta> {
        self.event_arrays.get(&id)
    }

    /// Resolve an array-select expression to its aggregate owner and
    /// declaration-relative member path, when Slang exposed that path through
    /// owned member references.
    pub fn array_select_path(&self, id: NodeId) -> Option<(NodeId, &[String])> {
        self.array_select_paths
            .get(&id)
            .map(|(owner, path)| (*owner, path.as_slice()))
    }

    pub fn var_initializers(&self) -> &HashMap<NodeId, NodeId> {
        &self.vars_init
    }

    pub fn var_initializer(&self, id: NodeId) -> Option<NodeId> {
        self.vars_init.get(&id).copied()
    }

    /// Return owned clocking block metadata, if `id` names a clocking block.
    pub fn clocking_block(&self, id: NodeId) -> Option<&ClockingBlockInfo> {
        self.clocking_blocks.get(&id)
    }

    /// Return owned clocking variable metadata, if `id` names a clocking
    /// block variable.
    pub fn clocking_var(&self, id: NodeId) -> Option<&ClockingVarInfo> {
        self.clocking_vars.get(&id)
    }

    pub fn is_clocking_block(&self, id: NodeId) -> bool {
        self.clocking_blocks.contains_key(&id)
    }

    pub fn is_clocking_var(&self, id: NodeId) -> bool {
        self.clocking_vars.contains_key(&id)
    }

    /// Return the direction of one captured modport port, if the node is a
    /// modport-port declaration rather than the enclosing view.
    pub fn modport_port_direction(&self, id: NodeId) -> Option<Direction> {
        self.modport_directions.get(&id).copied()
    }

    /// Return the concrete interface instance statically bound to a virtual
    /// interface variable. Runtime reassignment is intentionally not modeled.
    pub fn virtual_interface_target(&self, variable: NodeId) -> Option<NodeId> {
        self.virtual_interface_targets.get(&variable).copied()
    }

    /// Whether an expression is the initializer of a statically bound virtual
    /// interface variable. Such scope references are consumed by elaboration
    /// and must not be lowered as executable values.
    pub fn is_virtual_interface_initializer(&self, expression: NodeId) -> bool {
        self.virtual_interface_targets.keys().any(|variable| {
            self.vars_init
                .get(variable)
                .is_some_and(|initializer| *initializer == expression)
        })
    }

    /// Resolve a clocking block/clocking variable referenced through a
    /// statically initialized virtual interface handle. The returned identity
    /// is the concrete interface member captured in the owned database.
    pub fn resolve_clocking_member(&self, expression: NodeId) -> Option<NodeId> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.node_kind(expression) else {
            return None;
        };
        let variable = refs.iter().flatten().find_map(|reference| {
            self.virtual_interface_targets
                .contains_key(reference)
                .then_some(*reference)
        })?;
        let interface = self.virtual_interface_targets.get(&variable).copied()?;
        let expression_name = self.node(expression).name.as_str();
        let name = if expression_name.is_empty() {
            parts.last().map(String::as_str).unwrap_or_default()
        } else {
            expression_name
        };
        if name.is_empty() {
            return None;
        }
        let mut pending = vec![interface];
        let mut visited = HashSet::new();
        while let Some(owner) = pending.pop() {
            if !visited.insert(owner) {
                continue;
            }
            for child in self.node(owner).children.iter().copied() {
                if self.node(child).name == name
                    && (self.is_clocking_block(child) || self.is_clocking_var(child))
                {
                    return Some(child);
                }
                if matches!(
                    self.node_kind(child),
                    NodeKind::ModuleInst { .. }
                        | NodeKind::Stmt(StmtKind::Begin)
                        | NodeKind::GenScope
                        | NodeKind::GenScopeArray
                ) {
                    pending.push(child);
                }
            }
        }
        None
    }

    /// Return the propagation delay declared on a net symbol, if any.
    pub fn net_delays(&self) -> &HashMap<NodeId, DriverDelay> {
        &self.net_delays
    }

    pub fn net_delay(&self, id: NodeId) -> Option<DriverDelay> {
        self.net_delays.get(&id).copied()
    }

    /// Exact executable body attached to a function or task declaration.
    pub fn subroutine_body(&self, id: NodeId) -> Option<NodeId> {
        match self.node_kind(id) {
            NodeKind::FuncTask { body, .. } => *body,
            _ => None,
        }
    }

    /// Return the effective lifetime resolved by Slang for a variable.
    pub fn variable_lifetime(&self, id: NodeId) -> VariableLifetime {
        self.var_lifetimes
            .get(&id)
            .copied()
            .unwrap_or(VariableLifetime::Unavailable)
    }

    pub(crate) fn variable_lifetime_nodes(&self) -> &HashMap<NodeId, VariableLifetime> {
        &self.var_lifetimes
    }

    pub fn variable_lifetime_qualifier(&self, id: NodeId) -> VariableLifetimeQualifier {
        self.var_lifetime_qualifiers
            .get(&id)
            .copied()
            .unwrap_or(VariableLifetimeQualifier::Unavailable)
    }

    pub fn method_call_has_with_clause(&self, id: NodeId) -> bool {
        self.method_calls_with_clause.contains(&id)
    }

    /// Return the declaration bound to a method's implicit iterator, when the
    /// frontend supplied one for its `with` clause.
    pub fn method_call_iterator(&self, id: NodeId) -> Option<NodeId> {
        self.method_call_iterators.get(&id).copied()
    }

    pub(crate) fn method_calls_with_clause_nodes(&self) -> &HashSet<NodeId> {
        &self.method_calls_with_clause
    }

    pub(crate) fn method_call_iterator_nodes(&self) -> &HashMap<NodeId, NodeId> {
        &self.method_call_iterators
    }

    pub fn packed_members(&self, id: NodeId) -> Option<&[PackedMember]> {
        self.packed_members.get(&id).map(Vec::as_slice)
    }

    pub fn aggregate_layout(&self, id: NodeId) -> Option<&AggregateLayout> {
        self.aggregate_layouts.get(&id)
    }

    /// Return the complete recursive type descriptor captured for a
    /// declaration, when Slang supplied a type record for it.
    pub fn type_descriptor(&self, id: NodeId) -> Option<&TypeDescriptor> {
        self.type_descriptors.get(&id)
    }

    /// Return the owned declaration-order member table for an enum type.
    pub fn enum_type_metadata(&self, id: TypeId) -> Option<&EnumTypeMetadata> {
        self.enum_types.get(&id)
    }

    pub fn packed_dimensions(&self, id: NodeId) -> Option<&[PackedRange]> {
        self.packed_dimensions.get(&id).map(Vec::as_slice)
    }

    pub fn is_two_state_type(&self, id: NodeId) -> bool {
        self.two_state_types.contains(&id)
    }

    pub fn is_implicit_net(&self, id: NodeId) -> bool {
        self.implicit_nets.contains(&id)
    }

    pub fn is_implicit_conversion(&self, id: NodeId) -> bool {
        self.implicit_conversions.contains(&id)
    }

    /// Exact admitted source buffer for frontend-derived recovery logic.
    pub fn source_text(&self, path: &str) -> Option<&str> {
        self.source_files.get(path).map(String::as_str)
    }

    /// Instance path of a `module_inst` node (`"top.u0"`), `""` for the top.
    ///
    /// Built from the module-instance ancestor chain; generate scopes
    /// contribute their scope names (e.g. `"top.g[0].u"`), so per-iteration
    /// instances get distinct paths.  Each ancestor's name has any `lib@`
    /// prefix stripped.
    pub fn instance_path(&self, id: NodeId) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut cur = Some(id);
        while let Some(nid) = cur {
            let node = &self.nodes[nid.index()];
            if matches!(
                node.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                let name = node.name.clone();
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            cur = node.parent;
        }
        parts.reverse();
        if parts.len() <= 1 {
            // The top instance itself has no path.
            String::new()
        } else {
            parts.join(".")
        }
    }

    /// Return the owned packed-range projection captured by [`Self::from_slang`].
    pub fn elaborated_type_ranges(&self) -> &[ElaboratedTypeRanges] {
        &self.elaborated_type_ranges
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Complete language policy selected when this database was captured.
    pub fn edition(&self) -> LanguageEdition {
        self.edition
    }

    /// Whether Slang replaced this parameter's declaration initializer with
    /// an explicit elaboration override.
    pub fn parameter_is_overridden(&self, id: NodeId) -> bool {
        self.overridden_parameters.contains(&id)
    }
}
