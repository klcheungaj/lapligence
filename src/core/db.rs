//! Owned UHDM database facade.
//!
//! [`Db::build`] is the only VPI traversal entry point. Capture mechanics,
//! owned discriminants, and validation live in private submodules; consumers
//! receive a validated, fully owned snapshot through this facade.

mod capture;
mod database;
mod domain;
mod validate;

pub use database::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, ArrayMeta,
    AssignmentPatternKeyType, AssociativeIndex, CaseItem, ConstantSource, Db, DbError,
    ElaboratedTypeRanges, EventSpec, ExprKind, GateTerm, IntraControl, Node, NodeId, NodeKind,
    PackedMember, PackedRange, PrimClass, ProcessKind, StmtKind, VariableLifetimeQualifier,
};
pub use domain::{
    AlwaysKind, CaseKind, ConstantType, Direction, JoinKind, NetType, ObjectType, Operation,
    PrimitiveType, Strength,
};
pub use validate::DbValidationError;
