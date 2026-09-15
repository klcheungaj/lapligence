//! Lower non-integral values without encoding their storage as packed bits.
use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrClassFieldType, IrDisplayArg, IrEnumMember, IrEnumMethod, IrEnumQuery, IrExpr, IrMailboxElement,
    IrMailboxExpr, IrMailboxTarget, IrMailboxValue, IrObject, IrObjectQuery, IrObjectStmt,
    IrObjectType, IrProcessControl, IrProcessExpr, IrStringExpr,
};

mod assignments;
mod classes;
mod classification;
mod enumerations;
mod handles;
mod initialization;
mod mailboxes;
mod methods;
mod processes;
mod queries;
mod strings;
mod virtual_interfaces;


type VirtualInterfaceAccess = (IrChandleExpr, usize, usize, u32, bool, bool);

pub(super) fn object_query(query: IrObjectQuery, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::ObjectQuery(Box::new(query)),
        width,
        signed,
        None,
    )
}
