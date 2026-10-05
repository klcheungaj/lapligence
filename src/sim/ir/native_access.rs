//! Typed dynamic storage identities and class construction recipes.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct IrNativeAccess {
    pub(in crate::sim) name: String,
    pub(in crate::sim) receiver: IrChandleExpr,
    pub(in crate::sim) kind: IrNativeAccessKind,
    pub(in crate::sim) site: Option<String>,
    /// Item indices from a [`IrNativeAccessKind::ValueItem`] root to its leaf;
    /// empty for other kinds.
    pub(in crate::sim) item_path: Vec<u32>,
    /// Context of formal reads in the receiver expression.
    pub(in crate::sim) function: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrNativeAccessKind {
    ClassField {
        class: usize,
        field: usize,
    },
    InterfaceMember {
        interface: usize,
        member: usize,
    },
    /// One leaf of a descriptor-backed native value. The receiver is unused
    /// (`Null`); the value slot is resolved from the enclosing frame.
    ValueItem {
        value: usize,
        ty: IrClassFieldType,
    },
    /// One leaf, at `item_path`, of a record element of container storage.
    /// The receiver is an `IrChandleExpr::ContainerElement` locator,
    /// re-evaluated at every use; a write publishes the container contents
    /// change after the store (SV 7.5-7.10).
    ElementItem {
        ty: IrClassFieldType,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrClassAllocation {
    pub(in crate::sim) class: usize,
    pub(in crate::sim) local: String,
    pub(in crate::sim) body: Vec<IrStmt>,
    pub(in crate::sim) function: Option<usize>,
}
