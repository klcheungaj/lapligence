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
    /// The event object of the clocking block `event` of the interface
    /// instance the receiver (a virtual-interface handle) is bound to,
    /// readable as an event handle (SV 14.13, 25.9).
    InterfaceEvent {
        interface: usize,
        event: usize,
    },
    /// One leaf of a descriptor-backed native value. The receiver is unused
    /// (`Null`); the value slot is resolved from the enclosing frame.
    ValueItem {
        value: usize,
        ty: IrClassFieldType,
    },
    /// One leaf, at `item_path`, of a located value. The receiver is an
    /// `IrChandleExpr::ContainerElement` or `IrChandleExpr::ItemElement`
    /// locator, re-evaluated at every use; a write to a container element
    /// publishes the container contents change after the store (SV 7.5-7.10).
    /// The path is empty when the locator selects the leaf itself.
    ElementItem {
        ty: IrClassFieldType,
    },
    /// The change marker of the container property `field` of the object
    /// the receiver names; only a wait dependency, never storage. The
    /// container publishes to it from the object's construction once some
    /// wait in the model observes the property (SV 9.4.2).
    ClassContainer {
        class: usize,
        field: usize,
    },
    /// The contents marker of the native record property `field` of the
    /// object the receiver names; only a wait dependency, never storage.
    /// Every store into a leaf of the record, and every whole-record store,
    /// toggles the object's marker once some wait in the model observes the
    /// property: a toggle says that something in the record changed, so the
    /// waiter evaluates what it reads again (SV 9.4.2).
    ClassRecord {
        class: usize,
        field: usize,
    },
    /// The count change marker of the mailbox the receiver names, allocated
    /// when a wait first observes it; only a wait dependency (SV 15.4.2).
    MailboxCount,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrClassAllocation {
    pub(in crate::sim) class: usize,
    pub(in crate::sim) local: String,
    pub(in crate::sim) body: Vec<IrStmt>,
    pub(in crate::sim) function: Option<usize>,
}
