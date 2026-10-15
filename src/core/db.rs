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
    ConstantSource, ConstraintBlockInfo, ConstraintKind, Db, DbError, DpiImportInfo, DpiOpenType,
    DriverDelay, ElaboratedTypeRanges, EnumMember, EnumTypeMetadata, EventSpec, EventTriggerTiming,
    ExprKind, GateTerm, ImmediateAssertionKind, IntraControl, LegacyDirective, Node, NodeId,
    NodeKind, PackedMember, PackedRange, PredicateClause, PrimClass, ProcessKind, RandQualifier,
    RandomizeCall, RandomizeForm, StmtKind, StreamOperand, StreamingDirection, TimeLiteralScale,
    TimeUnit, TypeDescriptor, TypeId, TypeShape, UdpRow, UdpTable, ValueCopySemantics,
    ValueDefaultSemantics, ValueDestroySemantics, ValueEqualitySemantics, VariableLifetime,
    VariableLifetimeQualifier,
};
pub use domain::{
    AlwaysKind, CapturedSemanticKind, CaseKind, ConstantType, Direction, JoinKind, NetType,
    ObjectType, Operation, PrimitiveType, Strength, UnconnectedDrive, UniquePriorityCheck,
};
pub use source_map::{LogicalPosition, SourceMap};
pub use validate::DbValidationError;
