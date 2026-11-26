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
    /// The handle of event slot `event` (a clocking block or a named event)
    /// of the interface instance the receiver (a virtual-interface handle) is
    /// bound to, readable as an event handle (SV 14.13, 15.5, 25.9).
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
    /// A change marker of unpacked member storage slot `slot` (see
    /// [`crate::sim::ir::IrInterfaceStorage`]) of the interface instance the
    /// receiver, a virtual-interface handle, binds when the wait arms (SV
    /// 25.9); only a wait dependency, never storage. An unbound receiver
    /// selects storage that never changes.
    InterfaceStorage {
        interface: usize,
        slot: usize,
        part: IrInterfacePart,
    },
}

/// The change marker of interface member storage a wait names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrInterfacePart {
    /// Any value of the array or container.
    Contents,
    /// One cell of the array, in flattened storage order.
    Cell(u64),
    /// The size and membership of the container.
    Shape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrClassAllocation {
    pub(in crate::sim) class: usize,
    pub(in crate::sim) local: String,
    pub(in crate::sim) body: Vec<IrStmt>,
    pub(in crate::sim) function: Option<usize>,
}

impl IrModel {
    /// The native access named `name` and its table index. Access symbols are numbered by their
    /// table position (`_llg_access_<n>`, enforced by the validator), so the
    /// lookup reads the index instead of scanning the table: effect
    /// collection and emission look an access up at every statement that
    /// names it, and a scan made that quadratic in the model size.
    /// Whether `addr` is the address of a virtual-interface member: signal
    /// storage of the bound interface instance.
    pub(in crate::sim) fn is_interface_member_address(&self, addr: &str) -> bool {
        addr.strip_prefix('&')
            .and_then(|name| self.native_access(name))
            .is_some_and(|(_, access)| {
                matches!(access.kind, IrNativeAccessKind::InterfaceMember { .. })
            })
    }

    pub(in crate::sim) fn native_access(&self, name: &str) -> Option<(usize, &IrNativeAccess)> {
        let index = name.strip_prefix("_llg_access_")?.parse::<usize>().ok()?;
        self.native_accesses
            .get(index)
            .filter(|access| access.name == name)
            .map(|access| (index, access))
    }
}
