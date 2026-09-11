//! Validated, owned semantic database facade.
//!
//! [`Db::from_slang`] is the only frontend import. Typed projection and
//! validation live in private submodules; consumers receive frontend-neutral
//! nodes and metadata with no native lifetime.

mod database;
mod domain;
mod slang_types;
mod validate;

pub use database::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, ArrayMeta,
    AssignmentPatternKeyType, AssociativeIndex, CaseItem, ConstantSource, Db, DbError, DriverDelay,
    ElaboratedTypeRanges, EventSpec, ExprKind, GateTerm, IntraControl, Node, NodeId, NodeKind,
    PackedMember, PackedRange, PrimClass, ProcessKind, StmtKind, StreamOperand, StreamingDirection,
    VariableLifetime, VariableLifetimeQualifier,
};
pub use domain::{
    AlwaysKind, CaseKind, ConstantType, Direction, JoinKind, NetType, ObjectType, Operation,
    PrimitiveType, Strength,
};
pub use validate::DbValidationError;
