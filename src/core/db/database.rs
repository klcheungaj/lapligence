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
    JoinKind, NetType, ObjectType, Operation, PrimitiveType, Strength, UnconnectedDrive,
    UniquePriorityCheck,
};

use crate::core::elab::Val;
use crate::core::model::TypeInfo;
use crate::core::value::ValueData;
use crate::ffi::slang::{
    ConstantValue as SlangConstantValue, DelaySelection, LanguageEdition, SemanticDefinitionKind,
    SemanticDriveStrength, SemanticEdgeRole, SemanticKind, SemanticNode, SemanticOperation,
    SemanticTimeScale, SemanticTimeUnit, Snapshot as SlangSnapshot, CLOCKING_BLOCK_DEFAULT,
    CLOCKING_BLOCK_GLOBAL, CLOCKING_EDGE_MASK, CLOCKING_INPUT_EDGE_SHIFT,
    CLOCKING_OUTPUT_EDGE_SHIFT, CLOCKING_VAR_OUTPUT_EDGE_SHIFT, SEMANTIC_ASSERTION_ABORT_REJECT,
    SEMANTIC_ASSERTION_ABORT_SYNC, SEMANTIC_ASSERTION_DEFERRED, SEMANTIC_ASSERTION_FINAL,
    SEMANTIC_ASSERTION_RANGE, SEMANTIC_ASSERTION_REPETITION, SEMANTIC_ASSERTION_STRONG,
    SEMANTIC_EXPR_CLOCKING_EVENT, SEMANTIC_EXPR_COPY_CLASS, SEMANTIC_EXPR_DIST,
    SEMANTIC_EXPR_TAGGED_UNION, SEMANTIC_EXPR_UPDATE_CURRENT, SEMANTIC_SCOPE_CLOCKING_BLOCK,
    SEMANTIC_STMT_CONCURRENT_ASSERT, SEMANTIC_STMT_CONCURRENT_ASSUME,
    SEMANTIC_STMT_CONCURRENT_COVER, SEMANTIC_STMT_CONCURRENT_COVER_SEQUENCE,
    SEMANTIC_STMT_CONCURRENT_EXPECT, SEMANTIC_STMT_CONCURRENT_RESTRICT,
    SEMANTIC_STMT_IMMEDIATE_ASSERT, SEMANTIC_STMT_IMMEDIATE_ASSUME, SEMANTIC_STMT_IMMEDIATE_COVER,
    SEMANTIC_TIMING_CYCLE_DELAY, SEMANTIC_TIMING_ONE_STEP_DELAY, SEMANTIC_VARIABLE_CLOCKING,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

mod types;
use types::TypedefNames;
pub use types::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, ArrayMeta,
    AssignmentPatternKeyType, AssociativeIndex, ClassMetadata, DpiImportInfo, DpiOpenType,
    ElaboratedTypeRanges, EnumMember, EnumTypeMetadata, PackedMember, PackedRange, TypeDescriptor,
    TypeId, TypeShape, TypedefRef, ValueCopySemantics, ValueDefaultSemantics,
    ValueDestroySemantics, ValueEqualitySemantics,
};
mod constraints;
use constraints::{constraint_block_from_slang, constraint_from_slang, ordered_targets};
pub use constraints::{
    ConstraintBlockInfo, ConstraintKind, RandQualifier, RandomizeCall, RandomizeForm,
};
mod randsequence;
use randsequence::production_from_slang;
pub use randsequence::{
    ProductionBinding, ProductionCaseItem, ProductionDecl, ProductionItem, ProductionKind,
    ProductionResult, ProductionRule,
};
mod nodes;
pub use nodes::{
    CaseItem, CheckerActual, CheckerInstanceInfo, GateTerm, Node, NodeKind, PatternCaseItem,
    PrimClass, ProcessKind, RandCaseItem, UdpLocation, UdpRow, UdpTable,
};
mod assertions;
pub use assertions::{
    AssertionBinaryOp, AssertionBinding, AssertionCaseItem, AssertionExprKind, AssertionRange,
    AssertionRepetition, AssertionRepetitionKind, AssertionUnaryOp, ConcurrentAssertionKind,
    ImmediateAssertionKind,
};
mod statements;
pub use statements::{
    ClockingBlockInfo, ClockingEdge, ClockingSkew, ClockingVarInfo, DriverDelay, EventSpec,
    EventTriggerTiming, IntraControl, StmtKind,
};
mod predicates;
use predicates::{conditional_branches_from_slang, predicate_from_slang};
pub use predicates::{
    ConditionalPatternField, ConditionalPatternInfo, ConditionalPatternKind, ConditionalPredicate,
    PredicateClause,
};
mod expressions;
pub use expressions::{
    ConstantSource, DistItem, ExprKind, StreamOperand, StreamingDirection, TimeLiteralScale,
    TimeUnit, VariableLifetime, VariableLifetimeQualifier,
};
mod references;
use references::{
    array_select_from_slang, call_member_access, canonical_reference_target, edge_target,
    edge_target_at, edge_targets, expression_reference_target, hierarchical_reference_target,
    member_path_from_slang, resolved_edge_target, semantic_edges, semantic_id, SemanticIds,
};
mod clocking;
use clocking::{
    clocking_block_from_expression, clocking_skew_from_slang, clocking_source_from_expression,
    virtual_interface_instance_from_slang,
};
mod connections;
use connections::{
    connection_source_expression, direction_from_slang, driver_delay, peel_gate_terminal,
    port_internal_expression,
};
mod values;
use values::{
    net_type_from_subkind, operation_from_slang, primitive_type_from_subkind, strength_from_slang,
    time_exponent, time_literal_scale, val_from_slang,
};
mod assertion_import;
use assertion_import::assertion_expr_from_slang;
mod node_import;
use node_import::{node_kind_from_slang, udp_tables_from_slang};
mod statement_import;
use statement_import::{event_specs, statement_from_slang};
mod expression_import;
use expression_import::{
    assign_semantic_full_names, enclosing_scope_name, expression_from_slang, source_position,
    SourcePositions,
};

mod capture;
mod coverage;
use coverage::CoverageImport;
pub use coverage::{
    CoverageBinInfo, CoverageBinKind, CoverageBlockEvent, CoverageCrossInfo, CoverageEvent,
    CoverageRepeat, CoverageSelect, CoverageSelectKind, CoverageTables, CoverageTransRange,
    CovergroupInfo, CoverpointInfo,
};
mod shared;
use shared::{DenseShared, DenseSharedBuilder};
mod fsm;
pub use fsm::{Fsm, FsmPragmaError, FsmStatePart};
mod specify;
use specify::{specify_tables_from_slang, SpecifyTables};
pub use specify::{
    ModulePath, PathCondition, PathConnection, PathEdge, PathPolarity, PathPulse, PathPulseScope,
    PulseStyle, SpecifyItem, SpecifyItemKind, Specparam, TimingCheck, TimingCheckKind, TimingEdges,
    TimingEvent,
};

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

/// One active optional compiler directive the frontend accepts without
/// giving it semantics (IEEE 1364-2001 Annex D, IEEE 1800-2009 Annex E).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyDirective {
    /// Directive name without the leading backtick.
    pub name: String,
    pub file: String,
    /// One-based line and UTF-16 column of the directive token.
    pub line: u32,
    pub column: u32,
}

/// One element of a primitive instance array: of a connection as wide as
/// `count` elements, the element owns slice `slice`, counted from the least
/// significant one, which belongs to the right-hand indices of the array
/// range (IEEE 1800-2009 28.3.6). Multidimensional arrays are flattened with
/// the rightmost dimension varying fastest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimitiveArrayPosition {
    pub slice: u32,
    pub count: u32,
}

#[derive(Debug)]
pub struct Db {
    nodes: Vec<Node>,
    /// Complete language policy selected for the owned frontend snapshot.
    edition: LanguageEdition,
    /// The `min:typ:max` operand the frontend evaluated constants with.
    delay_selection: DelaySelection,
    overridden_parameters: HashSet<NodeId>,
    /// Native semantic categories retained for coverage checks when the
    /// frontend-neutral [`NodeKind`] intentionally has no direct variant.
    semantic_kinds: Vec<CapturedSemanticKind>,
    /// Native detail text retained alongside [`semantic_kinds`] for
    /// source-located diagnostics about otherwise unsupported nodes. Nodes
    /// share one copy of each distinct spelling.
    semantic_details: DenseShared<Box<str>>,
    /// Pattern kind and declaration identity captured from Slang. Pattern
    /// syntax remains a separate side table because `NodeKind` intentionally
    /// does not expose native frontend pattern variants.
    conditional_patterns: HashMap<NodeId, ConditionalPatternInfo>,
    /// Resolved field and nested-pattern roles for structure conditional
    /// patterns. These metadata edges are kept out of executable children.
    conditional_pattern_fields: HashMap<NodeId, Vec<ConditionalPatternField>>,
    /// Elaborated module-instance/definition IDs whose definition is a
    /// SystemVerilog program.  Program identity is kept as owned semantic
    /// metadata rather than inferred from names or source text so simulator
    /// scheduling can distinguish it after the Slang snapshot is released.
    program_instances: HashSet<NodeId>,
    /// Checker instances (IEEE 1800-2009 17.3). Each is a hierarchy instance
    /// ([`NodeKind::ModuleInst`]) whose children are the elaborated checker
    /// body; this table keeps its checker identity and context.
    checker_instances: HashMap<NodeId, CheckerInstanceInfo>,
    /// Checker-body expressions that a formal reference expanded to, with
    /// the checker instance whose formal it is (IEEE 1800-2009 17.3).
    checker_actuals: HashMap<NodeId, CheckerActual>,
    /// Definition-level pull behavior for omitted input ports, copied from
    /// the frontend's directive state while the snapshot is imported.
    unconnected_drives: HashMap<NodeId, UnconnectedDrive>,
    /// Position of each element of a primitive instance array (IEEE
    /// 1800-2009 28.3.5-28.3.6): the frontend binds every element to the
    /// whole connection expressions, so consumers that need the element's
    /// own part select (pass switches) derive it from this position.
    primitive_array_positions: HashMap<NodeId, PrimitiveArrayPosition>,
    /// Declared `(left, right)` of each instance array level (IEEE 1800-2009
    /// 37.11), keyed by its [`NodeKind::InstanceArray`] node.
    instance_array_ranges: HashMap<NodeId, (i32, i32)>,
    /// Keyword lines of statements whose range does not start at them: a
    /// written block whose `begin`/`fork` keyword is on another line than
    /// its range's start, and a task or function body list (no range) where
    /// it opens.
    keyword_lines: HashMap<NodeId, u32>,
    /// Source library of each elaborated instance (its selected definition's
    /// library), package and compilation unit, copied from the frontend's
    /// library binding so `%l` survives snapshot destruction.
    source_libraries: HashMap<NodeId, String>,
    /// Resolved `(unit, precision)` power-of-ten exponents of packages,
    /// compilation units and classes (a class inherits its declaring scope's
    /// scale; class nodes have no owned parent). Module instances carry
    /// theirs in [`NodeKind::ModuleInst`]; a scope without a captured scale
    /// uses the compilation default.
    declaration_time_scales: HashMap<NodeId, (i32, i32)>,
    /// The scope that declares each class (class nodes have no owned
    /// parent): the nearest enclosing class, package, compilation unit,
    /// module instance, generate block or definition.
    class_declaring_scopes: HashMap<NodeId, NodeId>,
    /// Logical `` `line`` positions, kept apart from the physical node
    /// positions that remain the diagnostic identity.
    source_map: super::SourceMap,
    /// Active optional charge/delay compatibility directives. The frontend
    /// consumes them without any semantic effect, so the owned database keeps
    /// their positions for consumers that must not accept them silently.
    legacy_directives: Vec<LegacyDirective>,
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
    /// Resizable-container shape of subroutine formals and results, keyed by
    /// the [`NodeKind::FuncArg`] or [`NodeKind::FuncTask`] node. Formals keep
    /// their argument identity, so the shape is recorded beside them.
    subroutine_arrays: HashMap<NodeId, ArrayMeta>,
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
    /// Variables and class properties declared `const` (SV 6.20.6).
    const_variables: HashSet<NodeId>,
    /// Class properties declared `rand` or `randc` (IEEE 1800-2009 18.4).
    rand_qualifiers: HashMap<NodeId, RandQualifier>,
    /// Randomize calls (18.6.1, 18.12) and their inline constraint blocks.
    randomize_calls: HashMap<NodeId, RandomizeCall>,
    method_calls_with_clause: HashSet<NodeId>,
    /// `const'(expr)` cast expressions (IEEE 1800-2009 6.24.1).
    const_casts: HashSet<NodeId>,
    /// Assertion expressions written in parentheses (16.14.3).
    parenthesized_assertions: HashSet<NodeId>,
    /// Method-call node → the frontend-owned iterator declaration used by its
    /// `with` expression.  Slang visits the expression itself as a structural
    /// child, but the implicit iterator variable is not an argument edge.
    method_call_iterators: HashMap<NodeId, NodeId>,
    /// Top-level packed struct/union layouts keyed by the declared object.
    packed_members: HashMap<NodeId, Vec<PackedMember>>,
    /// Structure/union category and members keyed by the declared object.
    aggregate_layouts: HashMap<NodeId, AggregateLayout>,
    /// DPI import formals declared with an unsized dimension.
    dpi_open_types: HashMap<NodeId, DpiOpenType>,
    /// Complete recursive type descriptors keyed by the declared object.
    /// Nodes of one frontend type share its single descriptor.
    type_descriptors: DenseShared<TypeDescriptor>,
    packed_pattern_elements: HashMap<NodeId, TypeDescriptor>,
    /// Resolved `T` of each `std::mailbox #(T)` specialization, keyed by the
    /// class type identity.
    mailbox_elements: HashMap<TypeId, TypeDescriptor>,
    /// Ordered enum members keyed by Slang's canonical type identity.
    enum_types: HashMap<TypeId, EnumTypeMetadata>,
    /// Typedef names of declared types and aggregate members (37.23).
    typedefs: TypedefNames,
    /// Immediate elements of packed arrays of structures, unions and enums,
    /// keyed by the array type identity.
    packed_array_elements: HashMap<TypeId, TypeDescriptor>,
    /// Ordered ranges of multidimensional packed declarations.
    packed_dimensions: HashMap<NodeId, Vec<PackedRange>>,
    /// True for declarations whose complete packed type has a two-state base.
    two_state_types: HashSet<NodeId>,
    /// Local assertion variables are materialized independently for each
    /// sequence/property attempt. Slang does not attach them to an instance
    /// scope, so retain their declaration identity in a side table instead of
    /// treating them as model-global storage.
    assertion_local_vars: HashSet<NodeId>,
    /// Direction metadata for assertion formal variables. Local assertion
    /// declarations have no direction; retaining this separate map lets
    /// lowering distinguish per-attempt input captures from output/inout
    /// formals without changing the stable [`NodeKind::Var`] shape.
    assertion_formal_directions: HashMap<NodeId, Direction>,
    /// Declaration assignments of local assertion variables (IEEE 1800-2009
    /// 16.10), kept apart from `vars_init`: they run once per attempt of the
    /// enclosing sequence or property instance, never at time zero.
    assertion_local_initializers: HashMap<NodeId, NodeId>,
    /// Cloned Slang declarations may retain a source-identity edge to their
    /// canonical formal declaration. This map is owned metadata, not a native
    /// AST relationship.
    source_identities: HashMap<NodeId, NodeId>,
    /// Clocking block declarations and their resolved clock events/skews.
    clocking_blocks: HashMap<NodeId, ClockingBlockInfo>,
    /// Functional-coverage declarations (IEEE 1800-2009 19).
    coverage: CoverageTables,
    /// Clocking block variables and the source signal each samples.
    clocking_vars: HashMap<NodeId, ClockingVarInfo>,
    /// Default clocking block in effect for each elaborated instance, as the
    /// frontend resolved it, including `default clocking id;` references.
    default_clockings: HashMap<NodeId, NodeId>,
    /// The `default disable iff` expression each elaborated instance declares
    /// itself (IEEE 1800-2009 16.15), keyed by instance.
    instance_default_disables: HashMap<NodeId, NodeId>,
    /// Direction of each captured modport port. This is kept separately from
    /// `NodeKind::ModPort` so the frontend-neutral node shape remains stable.
    modport_directions: HashMap<NodeId, Direction>,
    /// Port expression of each modport expression port `.p(expr)`
    /// (SV 25.5.4), keyed by the modport-port declaration. References to
    /// such a port keep the port as their target; consumers read or write
    /// this expression in the interface instance instead.
    modport_expressions: HashMap<NodeId, NodeId>,
    /// Internal port expression of a port that does not connect to one whole
    /// internal symbol: a bit/part select in a non-ANSI port reference or an
    /// explicit ANSI port expression (SV 23.2.2.1-23.2.2.2), keyed by port.
    port_expressions: HashMap<NodeId, NodeId>,
    /// Component ports of each multi-port (a concatenated port expression
    /// `.p({a, b})`, SV 23.2.2.1), most significant first. Each component
    /// is an ordinary port with its own direction and connections.
    multi_ports: HashMap<NodeId, Vec<NodeId>>,
    /// Statically initialized virtual-interface variables and their concrete
    /// interface instances. Runtime reassignment remains outside this map.
    virtual_interface_targets: HashMap<NodeId, NodeId>,
    /// DPI-C import contracts copied from the frontend syntax/flags.  This is
    /// a side table so synthetic test nodes and the existing NodeKind ABI do
    /// not need a lossy placeholder field.
    dpi_imports: HashMap<NodeId, DpiImportInfo>,
    /// C identifier of every subroutine named by a DPI-C export declaration,
    /// one entry per instance of the declaring scope. Only these can a
    /// context import reach, so only then can it write SystemVerilog storage
    /// (SV 35.5.3).
    dpi_exports: HashMap<NodeId, String>,
    /// Nets declared implicitly by Slang's semantic analysis.
    implicit_nets: HashSet<NodeId>,
    /// Context conversions inserted by Slang rather than written as casts.
    implicit_conversions: HashSet<NodeId>,
    /// Exact admitted source buffers keyed by their frontend file name.
    source_files: HashMap<String, String>,
    elaborated_type_ranges: Vec<ElaboratedTypeRanges>,
    /// Specify blocks, module paths, specify items and specparam facts.
    specify: SpecifyTables,
    /// FSMs declared by IEEE 1800-2009 40.4 pragmas, per instance.
    fsms: Vec<Fsm>,
    /// Malformed, misplaced or unresolved FSM pragmas.
    fsm_pragma_errors: Vec<FsmPragmaError>,
}

impl Db {
    #[cfg(test)]
    pub(super) fn empty_for_validation_test() -> Self {
        Self {
            nodes: Vec::new(),
            edition: LanguageEdition::SystemVerilog2009,
            delay_selection: DelaySelection::Typical,
            overridden_parameters: HashSet::new(),
            semantic_kinds: Vec::new(),
            semantic_details: DenseShared::default(),
            conditional_patterns: HashMap::new(),
            conditional_pattern_fields: HashMap::new(),
            program_instances: HashSet::new(),
            checker_instances: HashMap::new(),
            checker_actuals: HashMap::new(),
            unconnected_drives: HashMap::new(),
            primitive_array_positions: HashMap::new(),
            instance_array_ranges: HashMap::new(),
            keyword_lines: HashMap::new(),
            source_libraries: HashMap::new(),
            declaration_time_scales: HashMap::new(),
            class_declaring_scopes: HashMap::new(),
            source_map: super::SourceMap::default(),
            legacy_directives: Vec::new(),
            tops: Vec::new(),
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            class_metadata: HashMap::new(),
            design_name: "test".to_owned(),
            arrays: HashMap::new(),
            event_arrays: HashMap::new(),
            subroutine_arrays: HashMap::new(),
            array_select_paths: HashMap::new(),
            vars_init: HashMap::new(),
            net_delays: HashMap::new(),
            var_lifetimes: HashMap::new(),
            var_lifetime_qualifiers: HashMap::new(),
            const_variables: HashSet::new(),
            rand_qualifiers: HashMap::new(),
            randomize_calls: HashMap::new(),
            method_calls_with_clause: HashSet::new(),
            const_casts: HashSet::new(),
            parenthesized_assertions: HashSet::new(),
            method_call_iterators: HashMap::new(),
            packed_members: HashMap::new(),
            aggregate_layouts: HashMap::new(),
            dpi_open_types: HashMap::new(),
            type_descriptors: DenseShared::default(),
            packed_pattern_elements: HashMap::new(),
            mailbox_elements: HashMap::new(),
            enum_types: HashMap::new(),
            typedefs: TypedefNames::default(),
            packed_array_elements: HashMap::new(),
            packed_dimensions: HashMap::new(),
            two_state_types: HashSet::new(),
            assertion_local_vars: HashSet::new(),
            assertion_formal_directions: HashMap::new(),
            assertion_local_initializers: HashMap::new(),
            source_identities: HashMap::new(),
            clocking_blocks: HashMap::new(),
            coverage: CoverageTables::default(),
            clocking_vars: HashMap::new(),
            default_clockings: HashMap::new(),
            instance_default_disables: HashMap::new(),
            modport_directions: HashMap::new(),
            modport_expressions: HashMap::new(),
            port_expressions: HashMap::new(),
            multi_ports: HashMap::new(),
            virtual_interface_targets: HashMap::new(),
            dpi_imports: HashMap::new(),
            dpi_exports: HashMap::new(),
            implicit_nets: HashSet::new(),
            implicit_conversions: HashSet::new(),
            source_files: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
            specify: SpecifyTables::default(),
            fsms: Vec::new(),
            fsm_pragma_errors: Vec::new(),
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
            delay_selection: DelaySelection::Typical,
            overridden_parameters: HashSet::new(),
            semantic_kinds: Vec::new(),
            semantic_details: DenseShared::default(),
            conditional_patterns: HashMap::new(),
            conditional_pattern_fields: HashMap::new(),
            program_instances: HashSet::new(),
            checker_instances: HashMap::new(),
            checker_actuals: HashMap::new(),
            unconnected_drives: HashMap::new(),
            primitive_array_positions: HashMap::new(),
            instance_array_ranges: HashMap::new(),
            keyword_lines: HashMap::new(),
            source_libraries: HashMap::new(),
            declaration_time_scales: HashMap::new(),
            class_declaring_scopes: HashMap::new(),
            source_map: super::SourceMap::default(),
            legacy_directives: Vec::new(),
            tops,
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            class_metadata: HashMap::new(),
            design_name: design_name.into(),
            arrays,
            event_arrays: HashMap::new(),
            subroutine_arrays: HashMap::new(),
            array_select_paths: HashMap::new(),
            vars_init: HashMap::new(),
            net_delays: HashMap::new(),
            var_lifetimes: HashMap::new(),
            var_lifetime_qualifiers: HashMap::new(),
            const_variables: HashSet::new(),
            rand_qualifiers: HashMap::new(),
            randomize_calls: HashMap::new(),
            method_calls_with_clause: HashSet::new(),
            const_casts: HashSet::new(),
            parenthesized_assertions: HashSet::new(),
            method_call_iterators: HashMap::new(),
            packed_members: HashMap::new(),
            aggregate_layouts: HashMap::new(),
            dpi_open_types: HashMap::new(),
            type_descriptors: DenseShared::default(),
            packed_pattern_elements: HashMap::new(),
            mailbox_elements: HashMap::new(),
            enum_types: HashMap::new(),
            typedefs: TypedefNames::default(),
            packed_array_elements: HashMap::new(),
            packed_dimensions: HashMap::new(),
            two_state_types: HashSet::new(),
            assertion_local_vars: HashSet::new(),
            assertion_formal_directions: HashMap::new(),
            assertion_local_initializers: HashMap::new(),
            source_identities: HashMap::new(),
            clocking_blocks: HashMap::new(),
            coverage: CoverageTables::default(),
            clocking_vars: HashMap::new(),
            default_clockings: HashMap::new(),
            instance_default_disables: HashMap::new(),
            modport_directions: HashMap::new(),
            modport_expressions: HashMap::new(),
            port_expressions: HashMap::new(),
            multi_ports: HashMap::new(),
            virtual_interface_targets: HashMap::new(),
            dpi_imports: HashMap::new(),
            dpi_exports: HashMap::new(),
            implicit_nets: HashSet::new(),
            implicit_conversions: HashSet::new(),
            source_files: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
            specify: SpecifyTables::default(),
            fsms: Vec::new(),
            fsm_pragma_errors: Vec::new(),
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
        self.semantic_details.get(id).map(|detail| &**detail)
    }

    /// Return owned conditional-pattern metadata, if `id` is a captured
    /// pattern node. Synthetic test databases do not carry frontend pattern
    /// metadata and therefore return `None`.
    pub fn conditional_pattern(&self, id: NodeId) -> Option<ConditionalPatternInfo> {
        self.conditional_patterns.get(&id).copied()
    }

    /// Return resolved member roles for a structure conditional pattern.
    pub fn conditional_pattern_fields(&self, id: NodeId) -> Option<&[ConditionalPatternField]> {
        self.conditional_pattern_fields.get(&id).map(Vec::as_slice)
    }

    pub(crate) fn conditional_pattern_field_entries(
        &self,
    ) -> &HashMap<NodeId, Vec<ConditionalPatternField>> {
        &self.conditional_pattern_fields
    }

    /// Whether an owned variable is a lexical `.name` binding introduced by
    /// a conditional pattern. These temporaries use variable-shaped nodes for
    /// lowering, but they are not module or process signals for lint activity.
    pub fn is_conditional_pattern_binding(&self, id: NodeId) -> bool {
        self.conditional_patterns
            .values()
            .any(|pattern| pattern.binding == Some(id))
    }

    /// Whether an elaborated instance or definition has program-block
    /// semantics.  Synthetic test databases do not carry native definition
    /// metadata and therefore report `false`.
    pub fn is_program_instance(&self, id: NodeId) -> bool {
        self.program_instances.contains(&id)
    }

    /// Checker identity of an elaborated instance (IEEE 1800-2009 17.3).
    pub fn checker_instance(&self, id: NodeId) -> Option<&CheckerInstanceInfo> {
        self.checker_instances.get(&id)
    }

    /// Every checker instance in node order.
    pub fn checker_instances(&self) -> Vec<NodeId> {
        let mut instances = self.checker_instances.keys().copied().collect::<Vec<_>>();
        instances.sort_unstable_by_key(|id| id.0);
        instances
    }

    /// Whether any checker is instantiated, so designs without checkers skip
    /// every checker-specific pass.
    pub fn has_checker_instances(&self) -> bool {
        !self.checker_instances.is_empty()
    }

    /// The checker formal a checker-body expression expanded from, when it
    /// is the expansion of a formal reference (IEEE 1800-2009 17.3).
    pub fn checker_actual(&self, id: NodeId) -> Option<CheckerActual> {
        self.checker_actuals.get(&id).copied()
    }

    /// Library that bound an elaborated instance's definition, or that holds
    /// a package or compilation unit. Synthetic test databases have none.
    pub fn source_library(&self, id: NodeId) -> Option<&str> {
        self.source_libraries.get(&id).map(String::as_str)
    }

    /// Resolved `(unit, precision)` power-of-ten exponents of a package,
    /// compilation unit or class declaration, when the frontend recorded one
    /// for it or for the scope that declares the class.
    pub fn declaration_time_scale(&self, id: NodeId) -> Option<(i32, i32)> {
        self.declaration_time_scales.get(&id).copied()
    }

    /// The scope that declares class `id`: the nearest enclosing class,
    /// package, compilation unit (a `Begin` with the `CompilationUnit`
    /// detail), module instance, generate block or definition. Synthetic test
    /// databases have none.
    pub fn class_declaring_scope(&self, id: NodeId) -> Option<NodeId> {
        self.class_declaring_scopes.get(&id).copied()
    }

    /// Active `` `default_decay_time``, `` `default_trireg_strength`` and
    /// `` `delay_mode_*`` directives in source order per file.
    pub fn legacy_directives(&self) -> &[LegacyDirective] {
        &self.legacy_directives
    }

    /// The FSMs that IEEE 1800-2009 40.4 pragmas declare, one per state
    /// vector and instance, in instance capture order and then source order.
    pub fn fsms(&self) -> &[Fsm] {
        &self.fsms
    }

    /// The FSM pragmas that are malformed, misplaced or name no signal or
    /// parameter of their module, each reported once per definition.
    pub fn fsm_pragma_errors(&self) -> &[FsmPragmaError] {
        &self.fsm_pragma_errors
    }

    /// Logical (`` `line``-mapped) positions of admitted source lines.
    pub fn source_map(&self) -> &super::SourceMap {
        &self.source_map
    }

    /// Logical file and line of a node's start, when a `` `line`` directive
    /// maps it. Physical [`Node`] positions are unchanged by directives.
    pub fn logical_position(&self, id: NodeId) -> Option<super::LogicalPosition<'_>> {
        let node = self.nodes.get(id.index())?;
        self.source_map
            .logical_position(node.file.as_deref()?, node.line)
    }

    /// Return the definition-level pull value for omitted input ports on an
    /// elaborated instance.  Missing metadata means ordinary undriven behavior.
    /// The position of a primitive instance inside its instance array, or
    /// `None` for a single instance.
    pub fn primitive_array_position(&self, id: NodeId) -> Option<PrimitiveArrayPosition> {
        self.primitive_array_positions.get(&id).copied()
    }

    /// The declared `(left, right)` range of an instance array level
    /// ([`NodeKind::InstanceArray`]); a multidimensional array nests one
    /// level per dimension, outermost first.
    pub fn instance_array_range(&self, id: NodeId) -> Option<(i32, i32)> {
        self.instance_array_ranges.get(&id).copied()
    }

    /// The line of the `begin` or `fork` keyword of written block `id`
    /// (a [`StmtKind::Begin`] or [`StmtKind::Fork`] node) when it is not the
    /// node's own line: a block of one statement spans only that statement.
    pub fn block_keyword_line(&self, id: NodeId) -> Option<u32> {
        self.keyword_lines.get(&id).copied().filter(|_| {
            matches!(
                self.nodes.get(id.index()).map(Node::kind),
                Some(NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. }))
            )
        })
    }

    /// The line where task or function body list `id` (several statements,
    /// no range of its own) opens: the token after the subroutine name.
    pub fn subroutine_body_line(&self, id: NodeId) -> Option<u32> {
        self.keyword_lines
            .get(&id)
            .copied()
            .filter(|_| self.semantic_detail(id) == Some("List"))
    }

    pub fn unconnected_drive(&self, id: NodeId) -> UnconnectedDrive {
        self.unconnected_drives
            .get(&id)
            .copied()
            .unwrap_or(UnconnectedDrive::None)
    }

    pub(crate) fn semantic_metadata_lengths(&self) -> (usize, usize) {
        (
            self.semantic_kinds.len(),
            self.semantic_details.slot_count(),
        )
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

    /// Whether the design declares any DPI-C export, through which foreign
    /// code called by a context import can reach SystemVerilog subroutines.
    pub fn has_dpi_exports(&self) -> bool {
        !self.dpi_exports.is_empty()
    }

    /// The C identifier under which a subroutine is exported (SV 35.4,
    /// 35.7), when a DPI-C export declaration names it.
    pub fn dpi_export(&self, id: NodeId) -> Option<&str> {
        self.dpi_exports.get(&id).map(String::as_str)
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

    /// Resizable-container shape (dynamic, queue or associative) of a
    /// subroutine formal or function result.
    pub fn subroutine_array_meta(&self, id: NodeId) -> Option<&ArrayMeta> {
        self.subroutine_arrays.get(&id)
    }

    /// Resolve an array-select expression, or a `foreach` statement over a
    /// record member, to its aggregate owner and declaration-relative member
    /// path, when Slang exposed that path through owned member references.
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

    /// All owned clocking-block metadata keyed by declaration identity.
    pub fn clocking_blocks(&self) -> &HashMap<NodeId, ClockingBlockInfo> {
        &self.clocking_blocks
    }

    /// Covergroup type declarations (IEEE 1800-2009 19.3), keyed by node.
    pub fn covergroups(&self) -> &HashMap<NodeId, CovergroupInfo> {
        &self.coverage.groups
    }

    pub fn covergroup(&self, id: NodeId) -> Option<&CovergroupInfo> {
        self.coverage.groups.get(&id)
    }

    /// The covergroup declaration whose type is `type_id`.
    pub fn covergroup_for_type(&self, type_id: TypeId) -> Option<NodeId> {
        self.coverage.group_types.get(&type_id).copied()
    }

    pub fn coverpoint(&self, id: NodeId) -> Option<&CoverpointInfo> {
        self.coverage.points.get(&id)
    }

    pub fn coverage_bin(&self, id: NodeId) -> Option<&CoverageBinInfo> {
        self.coverage.bins.get(&id)
    }

    /// The covergroup that declares cross `id`.
    pub fn coverage_cross_group(&self, id: NodeId) -> Option<NodeId> {
        self.coverage.crosses.get(&id).map(|cross| cross.group)
    }

    /// Cross declaration `id` (IEEE 1800-2009 19.6).
    pub fn coverage_cross(&self, id: NodeId) -> Option<&CoverageCrossInfo> {
        self.coverage.crosses.get(&id)
    }

    /// All owned clocking-variable metadata keyed by declaration identity.
    pub fn clocking_vars(&self) -> &HashMap<NodeId, ClockingVarInfo> {
        &self.clocking_vars
    }

    /// The default clocking block in effect for elaborated instance `id`
    /// (IEEE 1800-2009 14.12), whether declared `default clocking` there or
    /// named by a `default clocking id;` item.
    pub fn default_clocking(&self, id: NodeId) -> Option<NodeId> {
        self.default_clockings.get(&id).copied()
    }

    /// All default clocking assignments keyed by instance.
    pub fn default_clockings(&self) -> &HashMap<NodeId, NodeId> {
        &self.default_clockings
    }

    /// The `default disable iff` expression declared in elaborated instance
    /// `id` itself, not in an enclosing scope (vpiDefaultDisableIff, IEEE
    /// 1800-2009 37.5, 37.6, 37.9).
    pub fn instance_default_disable(&self, id: NodeId) -> Option<NodeId> {
        self.instance_default_disables.get(&id).copied()
    }

    /// All instance `default disable iff` expressions keyed by instance.
    pub fn instance_default_disables(&self) -> &HashMap<NodeId, NodeId> {
        &self.instance_default_disables
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

    /// Return the port expression of a modport expression port `.p(expr)`.
    /// A simple modport port (no expression) returns `None`; its references
    /// already resolve to the interface member itself.
    pub fn modport_port_expression(&self, id: NodeId) -> Option<NodeId> {
        self.modport_expressions.get(&id).copied()
    }

    pub(crate) fn modport_expression_entries(&self) -> &HashMap<NodeId, NodeId> {
        &self.modport_expressions
    }

    /// Return the internal expression of a port whose port expression is a
    /// select or an explicit ANSI expression rather than one whole internal
    /// symbol (SV 23.2.2.1-23.2.2.2). Such a port has no `low` symbol: input
    /// connections write this expression in the child instance and output
    /// connections read it.
    pub fn port_internal_expression(&self, port: NodeId) -> Option<NodeId> {
        self.port_expressions.get(&port).copied()
    }

    pub(crate) fn port_expression_entries(&self) -> &HashMap<NodeId, NodeId> {
        &self.port_expressions
    }

    /// Return the component ports of a multi-port (a concatenated port
    /// expression, SV 23.2.2.1), most significant first; empty for any other
    /// node. The multi-port itself carries no connection: each component
    /// port is connected to its slice of the actual.
    pub fn multi_port_components(&self, port: NodeId) -> &[NodeId] {
        self.multi_ports.get(&port).map_or(&[], Vec::as_slice)
    }

    /// Return the multi-port that a component port belongs to.
    pub fn multi_port_of(&self, component: NodeId) -> Option<NodeId> {
        self.multi_ports
            .iter()
            .find(|(_, components)| components.contains(&component))
            .map(|(port, _)| *port)
    }

    pub(crate) fn multi_port_entries(&self) -> &HashMap<NodeId, Vec<NodeId>> {
        &self.multi_ports
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

    /// Whether a variable or class property is declared `const` (SV
    /// 6.20.6): after its initialization nothing may write it, including a
    /// method that modifies its value.
    pub fn is_const_variable(&self, id: NodeId) -> bool {
        self.const_variables.contains(&id)
    }

    /// Whether expression `id` is a `const'(expr)` cast (IEEE 1800-2009
    /// 6.24.1). A procedural concurrent assertion evaluates it when the
    /// assertion is queued instead of sampling it (16.15.6.1).
    pub fn is_const_cast(&self, id: NodeId) -> bool {
        self.const_casts.contains(&id)
    }

    /// Whether assertion expression `id` was written in parentheses. The
    /// frontend binds the parentheses away; a clocking event inside them
    /// does not flow out of them (IEEE 1800-2009 16.14.3).
    pub fn is_parenthesized_assertion(&self, id: NodeId) -> bool {
        self.parenthesized_assertions.contains(&id)
    }

    /// Random qualifier of a class property (IEEE 1800-2009 18.4).
    pub fn rand_qualifier(&self, id: NodeId) -> RandQualifier {
        self.rand_qualifiers
            .get(&id)
            .copied()
            .unwrap_or(RandQualifier::None)
    }

    pub(crate) fn randomize_call_entries(&self) -> impl Iterator<Item = (&NodeId, &RandomizeCall)> {
        self.randomize_calls.iter()
    }

    /// The randomize form and inline constraints of a randomize call node.
    pub fn randomize_call(&self, id: NodeId) -> Option<&RandomizeCall> {
        self.randomize_calls.get(&id)
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

    pub(crate) fn const_cast_nodes(&self) -> &HashSet<NodeId> {
        &self.const_casts
    }

    pub(crate) fn parenthesized_assertion_nodes(&self) -> &HashSet<NodeId> {
        &self.parenthesized_assertions
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

    /// The open-array type of a DPI import formal (SV 35.5.6.1), or `None`
    /// for every other declaration.
    pub fn dpi_open_type(&self, id: NodeId) -> Option<&DpiOpenType> {
        self.dpi_open_types.get(&id)
    }

    /// Return the complete recursive type descriptor captured for a
    /// declaration, when Slang supplied a type record for it.
    pub fn type_descriptor(&self, id: NodeId) -> Option<&TypeDescriptor> {
        self.type_descriptors.get(id)
    }

    /// Immediate packed-array element type for an assignment pattern.
    pub fn packed_pattern_element(&self, id: NodeId) -> Option<&TypeDescriptor> {
        self.packed_pattern_elements.get(&id)
    }

    /// Element type of a `std::mailbox #(T)` specialization, resolved in the
    /// specializing scope. `None` for every other type.
    pub fn mailbox_element(&self, id: TypeId) -> Option<&TypeDescriptor> {
        self.mailbox_elements.get(&id)
    }

    /// Return the owned declaration-order member table for an enum type.
    pub fn enum_type_metadata(&self, id: TypeId) -> Option<&EnumTypeMetadata> {
        self.enum_types.get(&id)
    }

    /// The typedef the declared type of `id` names (IEEE 1800-2009 37.23):
    /// for a variable, net or parameter its declared type, for a typedef
    /// declaration the typedef it aliases, for an explicit cast its target
    /// type. `None` when no typedef is named.
    pub fn declared_typedef(&self, id: NodeId) -> Option<TypedefRef> {
        self.typedefs.declared.get(&id).copied()
    }

    /// The immediate element of a packed array type whose base is a packed
    /// structure, union or enum; its own descriptor is a flattened atom.
    pub fn packed_array_element(&self, array: TypeId) -> Option<&TypeDescriptor> {
        self.packed_array_elements.get(&array)
    }

    /// Whether typedef declaration `id` renames a built-in type keyword such
    /// as `time` (37.23 detail 6); its typedef alias is then that type.
    pub fn typedef_renames_builtin(&self, id: NodeId) -> bool {
        self.typedefs.builtin.contains(&id)
    }

    /// The typedef the declared type of member `member` (declaration order)
    /// of aggregate type `aggregate` names.
    pub fn member_typedef(&self, aggregate: TypeId, member: usize) -> Option<TypedefRef> {
        let member = u32::try_from(member).ok()?;
        self.typedefs.members.get(&(aggregate, member)).copied()
    }

    pub fn packed_dimensions(&self, id: NodeId) -> Option<&[PackedRange]> {
        self.packed_dimensions.get(&id).map(Vec::as_slice)
    }

    pub fn is_two_state_type(&self, id: NodeId) -> bool {
        self.two_state_types.contains(&id)
    }

    /// Whether `id` names a Slang-materialized local assertion variable.
    /// These declarations have per-attempt storage in sequence callbacks and
    /// must never be resolved as instance-global signals.
    pub fn is_assertion_local_var(&self, id: NodeId) -> bool {
        self.assertion_local_vars.contains(&id)
    }

    /// The declaration assignment of local assertion variable `id`, if any.
    pub fn assertion_local_initializer(&self, id: NodeId) -> Option<NodeId> {
        self.assertion_local_initializers.get(&id).copied()
    }

    /// Return the direction of an assertion formal variable when Slang
    /// materialized one. Ordinary sequence locals intentionally return
    /// `None` because they are not formals.
    pub fn assertion_formal_direction(&self, id: NodeId) -> Option<Direction> {
        if let Some(direction) = self.assertion_formal_directions.get(&id) {
            return Some(*direction);
        }
        let node = self.nodes.get(id.index())?;
        self.assertion_formal_directions
            .iter()
            .find_map(|(formal, direction)| {
                let candidate = self.nodes.get(formal.index())?;
                (candidate.name == node.name
                    && candidate.file == node.file
                    && candidate.line == node.line
                    && candidate.col == node.col
                    && candidate.end_line == node.end_line
                    && candidate.end_col == node.end_col)
                    .then_some(*direction)
            })
    }

    /// Follow Slang's owned source-identity alias for a cloned declaration or
    /// reference. Identity aliases are only used to correlate formal symbols;
    /// ordinary storage lookup continues to use the concrete node identity.
    pub fn source_identity(&self, id: NodeId) -> NodeId {
        let mut current = id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some(&representative) = self.source_identities.get(&current) else {
                break;
            };
            if representative == current {
                break;
            }
            current = representative;
        }
        current
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

    /// Hierarchical name segments of the module instance or generate scope
    /// `id`, root first, as SV 27.4 writes them: a loop generate block is named
    /// once as written (`g[0]`), not as the loop array followed by the block,
    /// and an unnamed block keeps the array's name. The top instance yields its
    /// own name. This is the one naming rule behind `%m`, `svScope` names and
    /// lint messages.
    pub fn hierarchical_name_parts(&self, id: NodeId) -> Vec<&str> {
        let mut parts: Vec<&str> = Vec::new();
        let mut named_block = false;
        let mut current = Some(id);
        while let Some(nid) = current {
            let node = &self.nodes[nid.index()];
            match node.kind {
                NodeKind::ModuleInst { .. } | NodeKind::GenScope => {
                    named_block = matches!(node.kind, NodeKind::GenScope) && !node.name.is_empty();
                    if !node.name.is_empty() {
                        parts.push(&node.name);
                    }
                }
                NodeKind::GenScopeArray => {
                    if !named_block && !node.name.is_empty() {
                        parts.push(&node.name);
                    }
                    named_block = false;
                }
                _ => {}
            }
            current = node.parent;
        }
        parts.reverse();
        parts
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

    /// The operand of every `min:typ:max` expression selected for this
    /// compilation (IEEE 1800-2009 11.12), shared by the frontend's constant
    /// evaluation and every consumer of the owned database.
    pub fn delay_selection(&self) -> DelaySelection {
        self.delay_selection
    }

    /// Whether Slang replaced this parameter's declaration initializer with
    /// an explicit elaboration override.
    pub fn parameter_is_overridden(&self, id: NodeId) -> bool {
        self.overridden_parameters.contains(&id)
    }
}

pub(super) use values::value_data_from_slang;
