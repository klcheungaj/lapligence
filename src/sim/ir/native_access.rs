//! Typed dynamic storage identities and class construction recipes.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct IrNativeAccess {
    pub(in crate::sim) name: String,
    pub(in crate::sim) receiver: IrChandleExpr,
    pub(in crate::sim) kind: IrNativeAccessKind,
    pub(in crate::sim) site: Option<String>,
    /// Context of formal reads in the receiver expression.
    pub(in crate::sim) function: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrNativeAccessKind {
    ClassField { class: usize, field: usize },
    InterfaceMember { interface: usize, member: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrClassAllocation {
    pub(in crate::sim) class: usize,
    pub(in crate::sim) local: String,
    pub(in crate::sim) body: Vec<IrStmt>,
    pub(in crate::sim) function: Option<usize>,
}
