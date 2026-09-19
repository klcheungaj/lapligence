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
    SEMANTIC_STMT_CONCURRENT_EXPECT, SEMANTIC_STMT_IMMEDIATE_ASSERT,
    SEMANTIC_STMT_IMMEDIATE_ASSUME, SEMANTIC_STMT_IMMEDIATE_COVER, SEMANTIC_TIMING_CYCLE_DELAY,
    SEMANTIC_TIMING_ONE_STEP_DELAY, SEMANTIC_VARIABLE_CLOCKING,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

mod types;
pub use types::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, ArrayMeta,
    AssignmentPatternKeyType, AssociativeIndex, ClassMetadata, DpiImportInfo, ElaboratedTypeRanges,
    EnumMember, EnumTypeMetadata, PackedMember, PackedRange, TypeDescriptor, TypeId, TypeShape,
    ValueCopySemantics, ValueDefaultSemantics, ValueDestroySemantics, ValueEqualitySemantics,
};
mod nodes;
pub use nodes::{CaseItem, GateTerm, Node, NodeKind, PrimClass, ProcessKind};
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
mod expressions;
pub use expressions::{
    ConstantSource, ExprKind, StreamOperand, StreamingDirection, TimeLiteralScale, TimeUnit,
    VariableLifetime, VariableLifetimeQualifier,
};
mod references;
use references::{
    array_select_from_slang, canonical_reference_target, edge_target, edge_target_at, edge_targets,
    expression_reference_target, member_path_from_slang, resolved_edge_target, semantic_edges,
    semantic_id,
};
mod clocking;
use clocking::{
    clocking_block_from_expression, clocking_skew_from_slang, clocking_source_from_expression,
    virtual_interface_instance_from_slang,
};
mod connections;
use connections::{
    connection_source_expression, direction_from_slang, driver_delay, peel_gate_terminal,
};
mod values;
use values::{
    net_type_from_subkind, operation_from_slang, primitive_type_from_subkind, strength_from_slang,
    time_exponent, time_literal_scale, val_from_slang,
};
mod assertion_import;
use assertion_import::assertion_expr_from_slang;
mod node_import;
use node_import::node_kind_from_slang;
mod statement_import;
use statement_import::{event_specs, is_named_event_expression, statement_from_slang};
mod expression_import;
use expression_import::{
    enclosing_scope_name, expression_from_slang, semantic_full_name, source_position,
};

mod capture;

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
    /// Cloned Slang declarations may retain a source-identity edge to their
    /// canonical formal declaration. This map is owned metadata, not a native
    /// AST relationship.
    source_identities: HashMap<NodeId, NodeId>,
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
            assertion_local_vars: HashSet::new(),
            assertion_formal_directions: HashMap::new(),
            source_identities: HashMap::new(),
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
            assertion_local_vars: HashSet::new(),
            assertion_formal_directions: HashMap::new(),
            source_identities: HashMap::new(),
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

    /// All owned clocking-block metadata keyed by declaration identity.
    pub fn clocking_blocks(&self) -> &HashMap<NodeId, ClockingBlockInfo> {
        &self.clocking_blocks
    }

    /// All owned clocking-variable metadata keyed by declaration identity.
    pub fn clocking_vars(&self) -> &HashMap<NodeId, ClockingVarInfo> {
        &self.clocking_vars
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

    /// Whether `id` names a Slang-materialized local assertion variable.
    /// These declarations have per-attempt storage in sequence callbacks and
    /// must never be resolved as instance-global signals.
    pub fn is_assertion_local_var(&self, id: NodeId) -> bool {
        self.assertion_local_vars.contains(&id)
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

pub(super) use values::value_data_from_slang;
