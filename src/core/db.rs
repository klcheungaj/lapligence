//! Validated, owned semantic database facade.
//!
//! [`Db::from_slang`] is the only frontend import. Typed projection and
//! validation live in private submodules; consumers receive frontend-neutral
//! nodes and metadata with no native lifetime.

mod database;
mod domain;
mod slang_types;
mod source_map;
mod validate;

pub use database::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, ArrayMeta, AssertionBinaryOp,
    AssertionBinding, AssertionCaseItem, AssertionExprKind, AssertionRange, AssertionRepetition,
    AssertionRepetitionKind, AssertionUnaryOp, AssignmentPatternKeyType, AssociativeIndex,
    CaseItem, ClockingBlockInfo, ClockingEdge, ClockingSkew, ClockingVarInfo,
    ConcurrentAssertionKind, ConditionalPatternInfo, ConditionalPatternKind, ConditionalPredicate,
    ConstantSource, ConstraintBlockInfo, ConstraintKind, CoverageBinInfo, CoverageBinKind,
    CoverageBlockEvent, CoverageCrossInfo, CoverageEvent, CoverageRepeat, CoverageSelect,
    CoverageSelectKind, CoverageTables, CoverageTransRange, CovergroupInfo, CoverpointInfo, Db,
    DbError, DistItem, DpiImportInfo, DpiOpenType, DriverDelay, ElaboratedTypeRanges, EnumMember,
    EnumTypeMetadata, EventSpec, EventTriggerTiming, ExprKind, GateTerm, ImmediateAssertionKind,
    IntraControl, LegacyDirective, ModulePath, Node, NodeId, NodeKind, PackedMember, PackedRange,
    PathCondition, PathConnection, PathEdge, PathPolarity, PredicateClause, PrimClass, ProcessKind,
    ProductionBinding, ProductionCaseItem, ProductionDecl, ProductionItem, ProductionKind,
    ProductionResult, ProductionRule, PulseStyle, RandQualifier, RandomizeCall, RandomizeForm,
    SpecifyItem, SpecifyItemKind, Specparam, StmtKind, StreamOperand, StreamingDirection,
    TimeLiteralScale, TimeUnit, TimingCheckKind, TypeDescriptor, TypeId, TypeShape, UdpLocation,
    UdpRow, UdpTable, ValueCopySemantics, ValueDefaultSemantics, ValueDestroySemantics,
    ValueEqualitySemantics, VariableLifetime, VariableLifetimeQualifier,
};
pub use domain::{
    AlwaysKind, CapturedSemanticKind, CaseKind, ConstantType, Direction, JoinKind, NetType,
    ObjectType, Operation, PrimitiveType, Strength, UnconnectedDrive, UniquePriorityCheck,
};
pub use source_map::{LogicalPosition, SourceMap};
pub use validate::DbValidationError;
